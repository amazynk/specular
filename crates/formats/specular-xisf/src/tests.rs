use super::*;

/// A minimal monolithic XISF writer: `xml` holds `@N@` placeholders for the offsets of the
/// attached blocks, filled in zero-padded to a fixed width so the header size is known.
fn build(xml: &str, blocks: &[&[u8]]) -> Vec<u8> {
    let head_len = {
        let mut x = xml.to_owned();
        for i in 0..blocks.len() {
            x = x.replace(&format!("@{i}@"), &format!("{:012}", 0));
        }
        x.len()
    };
    let mut offsets = Vec::new();
    let mut pos = 16 + head_len;
    for b in blocks {
        pos = pos.div_ceil(8) * 8;
        offsets.push(pos);
        pos += b.len();
    }
    let mut x = xml.to_owned();
    for (i, o) in offsets.iter().enumerate() {
        x = x.replace(&format!("@{i}@"), &format!("{o:012}"));
    }
    assert_eq!(x.len(), head_len);
    let mut out = b"XISF0100".to_vec();
    out.extend_from_slice(&(x.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(x.as_bytes());
    for (o, b) in offsets.iter().zip(blocks) {
        out.resize(*o, 0);
        out.extend_from_slice(b);
    }
    out
}

fn doc(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xisf version=\"1.0\" \
         xmlns=\"http://www.pixinsight.com/xisf\">{body}</xisf>"
    )
}

/// One attached image; `attrs` are extra attributes.
fn one_image(geometry: &str, sample: &str, attrs: &str, block: &[u8], children: &str) -> Vec<u8> {
    let xml = doc(&format!(
        "<Image geometry=\"{geometry}\" sampleFormat=\"{sample}\" {attrs} \
         location=\"attachment:@0@:{}\">{children}</Image>",
        block.len()
    ));
    build(&xml, &[block])
}

fn first(data: &[u8]) -> Image {
    parse(data).unwrap().images.remove(0)
}

fn le_u16(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// A zlib stream of stored (uncompressed) DEFLATE blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(65535).collect()
    };
    for (i, c) in chunks.iter().enumerate() {
        out.push(u8::from(i + 1 == chunks.len()));
        let n = c.len() as u16;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&(!n).to_le_bytes());
        out.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

/// A raw LZ4 block holding only literals.
fn lz4_literals(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let n = data.len();
    if n < 15 {
        out.push((n as u8) << 4);
    } else {
        out.push(0xF0);
        let mut r = n - 15;
        while r >= 255 {
            out.push(255);
            r -= 255;
        }
        out.push(r as u8);
    }
    out.extend_from_slice(data);
    out
}

fn shuffle(src: &[u8], item: usize) -> Vec<u8> {
    let count = src.len() / item;
    let mut out = Vec::with_capacity(src.len());
    for j in 0..item {
        for i in 0..count {
            out.push(src[i * item + j]);
        }
    }
    out.extend_from_slice(&src[count * item..]);
    out
}

#[test]
fn sniffs() {
    assert!(sniff(b"XISF0100\0\0\0\0"));
    assert!(!sniff(b"XISB0100"));
    assert!(!sniff(b"XISF"));
    assert_eq!(parse(b"SIMPLE  =").unwrap_err(), Error::NotXisf);
}

#[test]
fn planar_u16_gray() {
    let v: Vec<u16> = (0..12u16).map(|i| i * 1000).collect();
    let f = one_image("4:3:1", "UInt16", "colorSpace=\"Gray\"", &le_u16(&v), "");
    let img = first(&f);
    assert_eq!(
        (img.width, img.height, img.depth, img.channels),
        (4, 3, 1, 1)
    );
    assert_eq!(img.geometry, vec![4, 3]);
    assert_eq!(img.storage, Storage::Planar);
    assert_eq!(img.color_space, ColorSpace::Gray);
    assert!(matches!(img.pixel_bytes(&f).unwrap(), Cow::Borrowed(_)));
    let p = img.plane_f32(&f, 0, 0).unwrap();
    assert_eq!(p, v.iter().map(|&x| f32::from(x)).collect::<Vec<_>>());
    assert!(img.plane_f32(&f, 1, 0).is_err());
    assert!(img.plane_f32(&f, 0, 1).is_err());
}

#[test]
fn normal_rgb_u8() {
    // 3×2 pixels, RGB interleaved: R = i, G = 100 + i, B = 200 + i.
    let mut block = Vec::new();
    for i in 0..6u8 {
        block.extend_from_slice(&[i, 100 + i, 200 + i]);
    }
    let f = one_image(
        "3:2:3",
        "UInt8",
        "pixelStorage=\"Normal\" colorSpace=\"RGB\"",
        &block,
        "",
    );
    let img = first(&f);
    assert_eq!(img.storage, Storage::Normal);
    assert_eq!(img.color_space, ColorSpace::Rgb);
    for c in 0..3u32 {
        let p = img.plane_f32(&f, c, 0).unwrap();
        let want: Vec<f32> = (0..6).map(|i| (c * 100 + i) as f32).collect();
        assert_eq!(p, want, "channel {c}");
    }
    let all = img.decode_all_f32(&f).unwrap();
    assert_eq!(all.len(), 18);
    assert_eq!(&all[6..12], &[100., 101., 102., 103., 104., 105.]);
}

#[test]
fn planar_rgb_u16() {
    let v: Vec<u16> = (0..12u16).map(|i| i * 7).collect(); // 2×2 × 3 channels
    let f = one_image("2:2:3", "UInt16", "colorSpace=\"RGB\"", &le_u16(&v), "");
    let img = first(&f);
    assert_eq!(img.plane_f32(&f, 2, 0).unwrap(), vec![56., 63., 70., 77.]);
}

#[test]
fn every_sample_format_both_byte_orders() {
    let vals = [0.0f64, 1.0, 2.0, 250.0, 17.0, 3.0];
    for big in [false, true] {
        for s in [
            SampleFormat::UInt8,
            SampleFormat::UInt16,
            SampleFormat::UInt32,
            SampleFormat::UInt64,
            SampleFormat::Float32,
            SampleFormat::Float64,
        ] {
            let mut b = Vec::new();
            for &x in &vals {
                macro_rules! put {
                    ($v:expr) => {
                        if big {
                            b.extend_from_slice(&$v.to_be_bytes())
                        } else {
                            b.extend_from_slice(&$v.to_le_bytes())
                        }
                    };
                }
                match s {
                    SampleFormat::UInt8 => b.push(x as u8),
                    SampleFormat::UInt16 => put!(x as u16 * 3),
                    SampleFormat::UInt32 => put!(x as u32 * 100_000),
                    SampleFormat::UInt64 => put!(x as u64 * 1_000_000),
                    SampleFormat::Float32 => put!((x / 256.0) as f32),
                    SampleFormat::Float64 => put!(x / 256.0),
                }
            }
            let order = if big { "byteOrder=\"big\"" } else { "" };
            let f = one_image("3:2:1", s.name(), order, &b, "");
            let img = first(&f);
            assert_eq!(img.sample, s);
            assert_eq!(img.byte_order == ByteOrder::Big, big);
            let p = img.plane_f32(&f, 0, 0).unwrap();
            let want: Vec<f32> = vals
                .iter()
                .map(|&x| match s {
                    SampleFormat::UInt8 => x as f32,
                    SampleFormat::UInt16 => (x * 3.0) as f32,
                    SampleFormat::UInt32 => (x * 100_000.0) as f32,
                    SampleFormat::UInt64 => (x * 1_000_000.0) as f32,
                    _ => (x / 256.0) as f32,
                })
                .collect();
            assert_eq!(p, want, "{} big={big}", s.name());
        }
    }
}

#[test]
fn cube_planar_and_normal() {
    // 2×2 pixels, 3 slices, 2 channels; value = 100c + 10z + i.
    let val = |c: u32, z: u32, i: u32| (100 * c + 10 * z + i) as u16;
    let mut planar = Vec::new();
    for c in 0..2 {
        for z in 0..3 {
            for i in 0..4 {
                planar.push(val(c, z, i));
            }
        }
    }
    let mut normal = Vec::new();
    for z in 0..3 {
        for i in 0..4 {
            for c in 0..2 {
                normal.push(val(c, z, i));
            }
        }
    }
    for (storage, v) in [("Planar", planar), ("Normal", normal)] {
        let f = one_image(
            "2:2:3:2",
            "UInt16",
            &format!("pixelStorage=\"{storage}\""),
            &le_u16(&v),
            "",
        );
        let img = first(&f);
        assert_eq!((img.depth, img.channels), (3, 2));
        assert_eq!(img.geometry, vec![2, 2, 3]);
        for c in 0..2 {
            for z in 0..3 {
                let want: Vec<f32> = (0..4).map(|i| f32::from(val(c, z, i))).collect();
                assert_eq!(
                    img.plane_f32(&f, c, z).unwrap(),
                    want,
                    "{storage} c{c} z{z}"
                );
            }
        }
        let all = img.decode_all_f32(&f).unwrap();
        assert_eq!(all[(3 + 2) * 4 + 1], f32::from(val(1, 2, 1)));
    }
    // More than three dimensions multiply out into slices.
    let f = one_image("2:1:2:3:1", "UInt8", "", &[0; 12], "");
    assert_eq!(first(&f).depth, 6);
}

#[test]
fn inline_and_embedded_blocks() {
    let v = le_u16(&[1, 2, 3, 258]);
    let want = vec![1., 2., 3., 258.];
    // base64 of v: 01 00 02 00 03 00 02 01
    let b64 = "AQACAAMAAgE=";
    let hex = "0100020003000201";
    for (loc, body) in [
        ("inline:base64", b64.to_owned()),
        ("inline:hex", hex.to_owned()),
        ("inline:base64", "AQAC\n  AAMA\n  AgE=".to_owned()),
        (
            "embedded",
            format!("<Data encoding=\"base64\">{b64}</Data>"),
        ),
        ("embedded", format!("<Data encoding=\"hex\">{hex}</Data>")),
    ] {
        let xml = doc(&format!(
            "<Image geometry=\"2:2:1\" sampleFormat=\"UInt16\" location=\"{loc}\">{body}</Image>"
        ));
        let f = build(&xml, &[]);
        let img = first(&f);
        assert_eq!(img.plane_f32(&f, 0, 0).unwrap(), want, "{loc} {body}");
    }
    assert_eq!(v.len(), 8);
    // Embedded and compressed (the compression attribute on <Data>).
    let z = zlib_stored(&v);
    let enc: String = z.iter().map(|b| format!("{b:02x}")).collect();
    let xml = doc(&format!(
        "<Image geometry=\"2:2:1\" sampleFormat=\"UInt16\" location=\"embedded\">\
         <Data encoding=\"hex\" compression=\"zlib:8\">{enc}</Data></Image>"
    ));
    let f = build(&xml, &[]);
    let img = first(&f);
    assert_eq!(img.compression.as_ref().unwrap().codec, Codec::Zlib);
    assert_eq!(img.plane_f32(&f, 0, 0).unwrap(), want);
}

#[test]
fn keywords_and_properties() {
    let children = "<FITSKeyword name=\"OBJECT\" value=\"'M31     '\" comment=\"target\"/>\
        <FITSKeyword name=\"EXPTIME\" value=\"300.\" comment=\"[s]\"/>\
        <FITSKeyword name=\"HISTORY\" value=\"\" comment=\"stacked\"/>\
        <Property id=\"Instrument:ExposureTime\" type=\"Float32\" value=\"300\"/>\
        <Property id=\"Observation:Object:Name\" type=\"String\">M31</Property>\
        <Property id=\"A:Flag\" type=\"Boolean\" value=\"true\"/>\
        <Property id=\"Observation:Time:Start\" type=\"TimePoint\" value=\"2024-01-02T03:04:05Z\"/>\
        <Property id=\"A:Vec\" type=\"F64Vector\" length=\"3\" location=\"inline:base64\">AAAA</Property>\
        <Property id=\"A:V2\" type=\"F64Vector\" length=\"2\" location=\"inline:base64\">AAAAAAAA8D8AAAAAAAAAwA==</Property>\
        <Property id=\"A:F32\" type=\"F32Vector\" length=\"1\" location=\"inline:hex\">0000c03f</Property>\
        <Property id=\"A:M\" type=\"F64Matrix\" rows=\"1\" columns=\"2\" location=\"embedded\"><Data encoding=\"base64\">AAAAAAAA8D8AAAAAAAAAwA==</Data></Property>\
        <Property id=\"A:Str2\" type=\"String\" location=\"inline:hex\">4869</Property>";
    let xml = doc(&format!(
        "<Image geometry=\"1:1:1\" sampleFormat=\"UInt8\" location=\"attachment:@0@:1\" \
         imageType=\"Light\" offset=\"0.5\" orientation=\"flip\" id=\"main\">{children}</Image>\
         <Property id=\"File:Level\" type=\"Int32\" value=\"7\"/>\
         <Metadata><Property id=\"XISF:CreatorApplication\" type=\"String\">test</Property>\
         <Property id=\"bad\"/></Metadata>"
    ));
    let f = build(&xml, &[&[9]]);
    let x = parse(&f).unwrap();
    let img = &x.images[0];
    assert_eq!(img.fits_keywords.len(), 3);
    let obj = img.keyword("OBJECT").unwrap();
    assert_eq!(obj.text(), "M31");
    assert_eq!(obj.comment, "target");
    assert_eq!(img.keyword("EXPTIME").unwrap().text(), "300.");
    assert_eq!(img.keyword("HISTORY").unwrap().comment, "stacked");
    assert!(img.keyword("NAXIS").is_none());
    let pv = |id: &str| img.property(id).unwrap().value.clone();
    assert_eq!(pv("Instrument:ExposureTime"), PropertyValue::Number(300.0));
    assert_eq!(
        pv("Observation:Object:Name"),
        PropertyValue::String("M31".into())
    );
    assert_eq!(pv("A:Flag"), PropertyValue::Bool(true));
    assert_eq!(
        pv("Observation:Time:Start"),
        PropertyValue::TimePoint("2024-01-02T03:04:05Z".into())
    );
    // 3 bytes for 3 doubles: described, not read.
    assert_eq!(pv("A:Vec"), PropertyValue::Other("length=3".into()));
    assert_eq!(pv("A:V2"), PropertyValue::Vector(vec![1.0, -2.0]));
    assert_eq!(pv("A:F32"), PropertyValue::Vector(vec![1.5]));
    assert_eq!(
        pv("A:M"),
        PropertyValue::Matrix {
            rows: 1,
            columns: 2,
            values: vec![1.0, -2.0]
        }
    );
    assert_eq!(img.property("A:Vec").unwrap().kind, "F64Vector");
    assert_eq!(pv("A:Str2"), PropertyValue::String("Hi".into()));
    assert_eq!(img.image_type.as_deref(), Some("Light"));
    assert_eq!(img.offset.as_deref(), Some("0.5"));
    assert_eq!(img.orientation.as_deref(), Some("flip"));
    assert_eq!(img.id.as_deref(), Some("main"));
    // File level: root properties and <Metadata>; ones without a type are dropped.
    let ids: Vec<&str> = x.properties.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["File:Level", "XISF:CreatorApplication"]);
    assert_eq!(x.properties[0].value, PropertyValue::Number(7.0));
}

#[test]
fn references_resolve() {
    let xml = doc(
        "<Image geometry=\"2:1:1\" sampleFormat=\"UInt8\" location=\"attachment:@0@:2\">\
         <Reference ref=\"kw1\"/><Reference ref=\"missing\"/></Image>\
         <FITSKeyword uid=\"kw1\" name=\"TELESCOP\" value=\"'T1'\" comment=\"\"/>\
         <Metadata><Reference ref=\"p1\"/></Metadata>\
         <Property uid=\"p1\" id=\"X:Y\" type=\"UInt8\" value=\"4\"/>",
    );
    let f = build(&xml, &[&[1, 2]]);
    let x = parse(&f).unwrap();
    assert_eq!(x.images[0].keyword("TELESCOP").unwrap().text(), "T1");
    // The referenced property appears via <Metadata> and once more as a root property.
    assert!(x.properties.iter().all(|p| p.id == "X:Y"));
    assert_eq!(x.properties.len(), 2);
}

#[test]
fn multiple_images() {
    let xml = doc(
        "<Image geometry=\"1:1:1\" sampleFormat=\"UInt8\" location=\"attachment:@0@:1\"/>\
         <Image geometry=\"2:1:1\" sampleFormat=\"UInt16\" location=\"attachment:@1@:4\"/>",
    );
    let f = build(&xml, &[&[5], &le_u16(&[600, 700])]);
    let x = parse(&f).unwrap();
    assert_eq!(x.images.len(), 2);
    assert_eq!(x.images[0].plane_f32(&f, 0, 0).unwrap(), vec![5.]);
    assert_eq!(x.images[1].plane_f32(&f, 0, 0).unwrap(), vec![600., 700.]);
}

#[test]
fn lz4_blocks() {
    let mut out = Vec::new();
    // Literals only, short and with an extended length.
    codec::lz4_block(&lz4_literals(b"abc"), 3, &mut out).unwrap();
    assert_eq!(out, b"abc");
    let long: Vec<u8> = (0..300u32).map(|i| (i * 7) as u8).collect();
    out.clear();
    codec::lz4_block(&lz4_literals(&long), 300, &mut out).unwrap();
    assert_eq!(out, long);
    // "ab" then an overlapping match (offset 2, length 4 + 6 = 10), then literals "Z".
    let block = [0x26, b'a', b'b', 2, 0, 0x10, b'Z'];
    out.clear();
    codec::lz4_block(&block, 13, &mut out).unwrap();
    assert_eq!(out, b"ababababababZ");
    // A run: one literal, offset 1, match length 4 + 15 + 255 + 6 = 280 (extended).
    let block = [0x1F, b'x', 1, 0, 255, 6, 0x00];
    out.clear();
    codec::lz4_block(&block, 281, &mut out).unwrap();
    assert_eq!(out, vec![b'x'; 281]);
    // A non-overlapping match.
    let block = [0x40, b'w', b'x', b'y', b'z', 4, 0, 0x10, b'!'];
    out.clear();
    codec::lz4_block(&block, 9, &mut out).unwrap();
    assert_eq!(out, b"wxyzwxyz!");
    // Appends to what is already there, and offsets cannot reach into it.
    let mut pre = b"PRE".to_vec();
    codec::lz4_block(&lz4_literals(b"q"), 1, &mut pre).unwrap();
    assert_eq!(pre, b"PREq");
    let mut pre = b"PRE".to_vec();
    assert!(codec::lz4_block(&[0x00, 3, 0], 4, &mut pre).is_err());
    // Corrupt blocks.
    for (block, n) in [
        (&[0x26, b'a', b'b', 0, 0, 0x10, b'Z'][..], 13), // offset 0
        (&[0x26, b'a', b'b', 9, 0, 0x10, b'Z'], 13),     // offset past the start
        (&[0x26, b'a', b'b', 2, 0, 0x10, b'Z'], 12),     // overrun
        (&[0x26, b'a', b'b', 2, 0, 0x10, b'Z'], 14),     // short
        (&[0x30, b'a'], 3),                              // truncated literals
        (&[0x1F, b'x', 1], 300),                         // truncated offset
        (&[0x1F, b'x', 1, 0, 255], 300),                 // truncated length
        (&[], 0),
        (&[0xF0, 255, 255], 600), // truncated literal length
    ] {
        out.clear();
        assert!(
            codec::lz4_block(block, n, &mut out).is_err(),
            "{block:?} {n}"
        );
    }
}

#[test]
fn unshuffles() {
    let src: Vec<u8> = (0..1003u32).map(|i| (i * 31 % 251) as u8).collect();
    for item in [1usize, 2, 3, 4, 8] {
        assert_eq!(
            codec::unshuffle(&shuffle(&src, item), item),
            src,
            "item {item}"
        );
    }
    // Larger than one parallel chunk.
    let big: Vec<u8> = (0..(1u32 << 17) * 4 + 3)
        .map(|i| (i * 13 % 255) as u8)
        .collect();
    assert_eq!(codec::unshuffle(&shuffle(&big, 4), 4), big);
}

#[test]
fn zlib_and_lz4_with_shuffle_and_subblocks() {
    let v: Vec<u16> = (0..64u16).map(|i| i * 513).collect(); // 8×8
    let raw = le_u16(&v);
    let want: Vec<f32> = v.iter().map(|&x| f32::from(x)).collect();
    let sh = shuffle(&raw, 2);
    // zlib+sh, one block.
    let f = one_image(
        "8:8:1",
        "UInt16",
        "compression=\"zlib+sh:128:2\"",
        &zlib_stored(&sh),
        "",
    );
    let img = first(&f);
    assert_eq!(img.compression.as_ref().unwrap().shuffle_item, Some(2));
    assert_eq!(img.plane_f32(&f, 0, 0).unwrap(), want);
    // lz4, three sub-blocks.
    let parts = [&raw[..50], &raw[50..100], &raw[100..]];
    let blocks: Vec<Vec<u8>> = parts.iter().map(|p| lz4_literals(p)).collect();
    let sub = blocks
        .iter()
        .zip(parts)
        .map(|(c, p)| format!("{},{}", c.len(), p.len()))
        .collect::<Vec<_>>()
        .join(":");
    let f = one_image(
        "8:8:1",
        "UInt16",
        &format!("compression=\"lz4hc:128\" subblocks=\"{sub}\""),
        &blocks.concat(),
        "",
    );
    let img = first(&f);
    assert_eq!(img.compression.as_ref().unwrap().subblocks.len(), 3);
    assert_eq!(img.plane_f32(&f, 0, 0).unwrap(), want);
    // zlib+sh in sub-blocks (shuffled as a whole, then split).
    let z1 = zlib_stored(&sh[..64]);
    let z2 = zlib_stored(&sh[64..]);
    let f = one_image(
        "8:8:1",
        "UInt16",
        &format!(
            "compression=\"zlib+sh:128:2\" subblocks=\"{},64:{},64\"",
            z1.len(),
            z2.len()
        ),
        &[z1, z2].concat(),
        "",
    );
    assert_eq!(first(&f).plane_f32(&f, 0, 0).unwrap(), want);
}

/// The committed fixtures, written by the python `xisf` package (`gen/make_samples.py
/// --fixtures`): a 24×16 UInt16 image with every codec, with and without shuffling.
#[test]
fn python_fixtures() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let want: Vec<f32> = std::fs::read(format!("{dir}/expected_u16.bin"))
        .unwrap()
        .chunks_exact(2)
        .map(|b| f32::from(u16::from_le_bytes([b[0], b[1]])))
        .collect();
    assert_eq!(want.len(), 24 * 16);
    for (name, codec) in [
        ("zlib", Codec::Zlib),
        ("lz4", Codec::Lz4),
        ("lz4hc", Codec::Lz4Hc),
        ("zstd", Codec::Zstd),
    ] {
        for sh in [false, true] {
            let file = format!("{dir}/{name}{}.xisf", if sh { "_sh" } else { "" });
            let f = std::fs::read(&file).unwrap();
            let x = parse(&f).unwrap();
            assert_eq!(x.images.len(), 1);
            let img = &x.images[0];
            let c = img.compression.as_ref().unwrap();
            assert_eq!(c.codec, codec);
            assert_eq!(c.uncompressed, 768);
            assert_eq!(c.shuffle_item, sh.then_some(2));
            assert_eq!((img.width, img.height, img.channels), (24, 16, 1));
            assert_eq!(img.plane_f32(&f, 0, 0).unwrap(), want, "{file}");
            assert!(
                x.properties
                    .iter()
                    .any(|p| p.id == "XISF:CreatorApplication"
                        && p.value == PropertyValue::String("specular make_samples".into()))
            );
        }
    }
}

#[test]
fn corrupt_inputs_never_panic() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let mut files = vec![
        one_image(
            "4:3:1",
            "UInt16",
            "",
            &le_u16(&[7; 12]),
            "<FITSKeyword name=\"A\" value=\"1\"/>",
        ),
        one_image(
            "2:2:3:2",
            "Float64",
            "pixelStorage=\"Normal\"",
            &[1; 192],
            "",
        ),
    ];
    for n in ["zlib", "lz4_sh", "lz4hc", "zstd_sh"] {
        files.push(std::fs::read(format!("{dir}/{n}.xisf")).unwrap());
    }
    let try_all = |d: &[u8]| {
        if let Ok(x) = parse(d) {
            for img in &x.images {
                let _ = img.decode_all_f32(d);
            }
        }
    };
    for f in &files {
        for cut in 0..f.len() {
            try_all(&f[..cut]);
        }
        // Flip bytes across the file (header and data).
        let step = (f.len() / 300).max(1);
        for at in (0..f.len()).step_by(step) {
            for x in [0x01, 0x80, 0xFF] {
                let mut g = f.clone();
                g[at] ^= x;
                try_all(&g);
            }
        }
    }
}

#[test]
fn corrupt_inputs_are_errors() {
    let ok = le_u16(&[1, 2, 3, 4]);
    let bad = |geometry: &str, sample: &str, attrs: &str, block: &[u8]| {
        let f = one_image(geometry, sample, attrs, block, "");
        parse(&f).and_then(|x| x.images[0].decode_all_f32(&f).map(|_| ()))
    };
    // Geometry.
    for g in [
        "0:2:1",
        "2:2:0",
        "2",
        "",
        "a:b:1",
        "2:-2:1",
        "99999999999:1:1",
    ] {
        assert!(bad(g, "UInt16", "", &ok).is_err(), "{g}");
    }
    assert_eq!(bad("65536:65536:1", "UInt8", "", &ok), Err(Error::TooLarge));
    assert_eq!(
        bad("4096:4096:4096:4096:1", "UInt8", "", &ok),
        Err(Error::TooLarge)
    );
    // Samples and attributes.
    assert!(matches!(
        bad("2:2:1", "Complex32", "", &ok),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        bad("2:2:1", "Int16", "", &ok),
        Err(Error::Malformed(_))
    ));
    assert!(bad("2:2:1", "UInt16", "pixelStorage=\"Diagonal\"", &ok).is_err());
    assert!(bad("2:2:1", "UInt16", "colorSpace=\"CMYK\"", &ok).is_err());
    // Too little data.
    assert_eq!(
        bad("2:2:1", "UInt16", "", &ok[..7]),
        Err(Error::Truncated("pixel data"))
    );
    // Compression attributes and data.
    assert!(matches!(
        bad("2:2:1", "UInt16", "compression=\"lzma:8\"", &ok),
        Err(Error::Unsupported(_))
    ));
    assert!(bad("2:2:1", "UInt16", "compression=\"zlib+sh:8\"", &ok).is_err());
    assert!(bad("2:2:1", "UInt16", "compression=\"zlib:x\"", &ok).is_err());
    assert!(matches!(
        bad(
            "2:2:1",
            "UInt16",
            "compression=\"zlib:9\"",
            &zlib_stored(&[0; 9])
        ),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        bad("2:2:1", "UInt16", "compression=\"zlib:8\"", &ok),
        Err(Error::Codec(_))
    ));
    assert!(matches!(
        bad("2:2:1", "UInt16", "compression=\"zstd:8\"", &ok),
        Err(Error::Codec(_))
    ));
    assert!(matches!(
        bad(
            "2:2:1",
            "UInt16",
            "compression=\"lz4:8\"",
            &lz4_literals(&ok[..6])
        ),
        Err(Error::Codec(_))
    ));
    // Sub-blocks that disagree with the data.
    let z = zlib_stored(&ok);
    let n = z.len();
    for s in [
        format!("{n},4"),
        format!("{},8", n + 1),
        format!("{n},8:1,1"),
        "x".into(),
    ] {
        let attrs = format!("compression=\"zlib:8\" subblocks=\"{s}\"");
        assert!(bad("2:2:1", "UInt16", &attrs, &z).is_err(), "{s}");
    }
    // Locations.
    for (loc, body) in [
        ("attachment:999:8", ""),
        ("attachment:18446744073709551615:8", ""),
        ("attachment:16", ""),
        ("attachment", ""),
        ("url(http://example.invalid/a)", ""),
        ("inline:base85", "x"),
        ("inline:base64", "@@@@"),
        ("inline:hex", "abc"),
        ("embedded", ""),
    ] {
        let xml = doc(&format!(
            "<Image geometry=\"2:2:1\" sampleFormat=\"UInt16\" location=\"{loc}\">{body}</Image>"
        ));
        let f = build(&xml, &[]);
        let r = parse(&f).and_then(|x| x.images[0].decode_all_f32(&f).map(|_| ()));
        assert!(r.is_err(), "{loc}");
    }
    // Header.
    let mut f = one_image("2:2:1", "UInt16", "", &ok, "");
    f[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(parse(&f), Err(Error::TooLarge));
    let n = f.len() as u32;
    f[8..12].copy_from_slice(&n.to_le_bytes());
    assert!(matches!(parse(&f), Err(Error::Truncated(_))));
    assert!(matches!(
        parse(b"XISF0100\x05\0\0\0\0\0\0\0<a/>x"),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        parse(b"XISF0100\x04\0\0\0\0\0\0\0<a/>"),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        parse(b"XISF0100\x02\0\0\0\0\0\0\0\xff\xfe"),
        Err(Error::Malformed(_))
    ));
    assert_eq!(
        parse(b"XISF0100\x0a\0\0\0\0\0\0\0<xisf/>\0\0\0").unwrap(),
        Xisf::default()
    );
}

mod hostile {
    use super::*;

    /// Width × height × depth × channels overflowed u64 before the size check.
    #[test]
    fn geometry_product_overflow() {
        let f = one_image(
            "4294967295:4294967295:2:4294967295",
            "UInt8",
            "",
            &[0; 4],
            "",
        );
        assert_eq!(parse(&f), Err(Error::TooLarge));
    }
}

/// `n` references to keywords that exist and `n` to uids that do not, in one image.
fn many_references(n: usize) -> Vec<u8> {
    let mut body = String::from(
        "<Image geometry=\"2:1:1\" sampleFormat=\"UInt8\" location=\"attachment:@0@:2\">",
    );
    for i in 0..n {
        body += &format!("<Reference ref=\"k{i}\"/><Reference ref=\"m{i}\"/>");
    }
    body += "</Image>";
    for i in 0..n {
        body += &format!("<FITSKeyword uid=\"k{i}\" name=\"K{i}\" value=\"{i}\" comment=\"\"/>");
    }
    build(&doc(&body), &[&[1, 2]])
}

#[test]
fn hostile_many_references_are_indexed() {
    // 40000 resolved and 40000 dangling references: a document search per reference was
    // 80000 × 120000 node visits.
    let n = 40_000;
    let f = many_references(n);
    let t = std::time::Instant::now();
    let x = parse(&f).unwrap();
    let el = t.elapsed();
    assert_eq!(x.images[0].fits_keywords.len(), n);
    assert_eq!(x.images[0].fits_keywords[n - 1].name, format!("K{}", n - 1));
    assert!(el < std::time::Duration::from_secs(5), "{el:?}");
}

#[test]
fn references_resolve_to_the_first_uid() {
    // Duplicate uids resolve to the first element in document order, nested or not, and a
    // <Reference> carrying a uid is never a target.
    let xml = doc(
        "<Image geometry=\"2:1:1\" sampleFormat=\"UInt8\" location=\"attachment:@0@:2\">\
         <Reference ref=\"a\" uid=\"a\"/><Reference ref=\"b\"/></Image>\
         <Metadata><FITSKeyword uid=\"a\" name=\"FIRST\" value=\"1\" comment=\"\"/></Metadata>\
         <FITSKeyword uid=\"a\" name=\"SECOND\" value=\"2\" comment=\"\"/>\
         <FITSKeyword uid=\"b\" name=\"B\" value=\"3\" comment=\"\"/>",
    );
    let f = build(&xml, &[&[1, 2]]);
    let x = parse(&f).unwrap();
    let names: Vec<&str> = x.images[0]
        .fits_keywords
        .iter()
        .map(|k| k.name.as_str())
        .collect();
    assert_eq!(names, ["FIRST", "B"]);
}

/// 100,000 nested elements in the header: roxmltree recursed once per level and overflowed
/// the stack (the test thread's 256 KiB here). Refused before parsing now.
#[test]
fn deeply_nested_header() {
    let xml = doc(&format!(
        "{}{}",
        "<a>".repeat(100_000),
        "</a>".repeat(100_000)
    ));
    let f = build(&xml, &[]);
    let r = specular_thread::on_stack(256 << 10, || parse(&f).map(|_| ()));
    assert!(matches!(r, Err(Error::Malformed(_))), "{r:?}");
}

/// 4000 references to one property holding 64 KB of base64: each reference decoded the
/// block again (and kept a copy). References stop resolving past four times the header.
#[test]
fn references_to_one_large_block() {
    let b64 = "AAAA".repeat(16 << 10);
    let refs = "<Reference ref=\"big\"/>".repeat(4000);
    let xml = doc(&format!(
        "<Property uid=\"big\" id=\"A:B\" type=\"ByteArray\" length=\"49152\" \
         location=\"inline:base64\">{b64}</Property><Metadata>{refs}</Metadata>"
    ));
    let f = build(&xml, &[]);
    let x = parse(&f).unwrap();
    let n = x.properties.len();
    // At most 1 MiB (the allowance floor) of referenced elements: 16 copies, not 4001.
    assert!((2..=17).contains(&n), "{n} copies");
}

/// A 1 GiB image whose zstd block is 16 bytes: the buffer was reserved (and could be zeroed
/// past the machine's memory) before decompressing; it must fit the memory budget now.
#[test]
fn compressed_size_within_budget() {
    let f = one_image(
        "32768:32768:1",
        "UInt8",
        "compression=\"zstd:1073741824\"",
        &[0x28, 0xb5, 0x2f, 0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        "",
    );
    let img = first(&f);
    let r = specular_core::budget::with_limit(64 << 20, || img.plane_f32(&f, 0, 0));
    assert!(matches!(r, Err(Error::TooLarge)), "{r:?}");
}
