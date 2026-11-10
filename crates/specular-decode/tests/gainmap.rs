//! Gain-map JPEGs (Ultra HDR / Adobe hdrgm / ISO 21496-1), built synthetically: a tiny
//! writer splices XMP, MPF and ISO APP segments into two `jpeg-encoder` JPEGs.

use std::sync::Arc;

use jpeg_encoder::{ColorType, Encoder};
use specular_core::hdr::srgb_to_linear;
use specular_core::{CancelToken, GainMapParams, PixelLayout, Primaries, Raster, Source};
use specular_decode::{
    DecodeRequest, Registry, apple_gain_map, apple_headroom, jhgm_params, jpeg_gain_map,
    jpeg_has_gain_map, parse_hdrgm_xmp, parse_iso21496, tmap_params,
};

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const ISO_SIG: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";

fn encode(w: u16, h: u16, data: &[u8], ct: ColorType) -> Vec<u8> {
    let mut v = Vec::new();
    Encoder::new(&mut v, 90).encode(data, w, h, ct).unwrap();
    v
}

fn base_jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            px.extend_from_slice(&[(x * 4) as u8, (y * 4) as u8, 96]);
        }
    }
    encode(w, h, &px, ColorType::Rgb)
}

/// A grey gain map: a horizontal ramp 0..=255.
fn map_jpeg(w: u16, h: u16) -> Vec<u8> {
    let px: Vec<u8> = (0..h)
        .flat_map(|_| (0..w).map(move |x| (x as u32 * 255 / (w as u32 - 1)) as u8))
        .collect();
    encode(w, h, &px, ColorType::Luma)
}

fn segment(marker: u8, sig: &[u8], body: &[u8]) -> Vec<u8> {
    let len = 2 + sig.len() + body.len();
    let mut s = vec![0xFF, marker, (len >> 8) as u8, len as u8];
    s.extend_from_slice(sig);
    s.extend_from_slice(body);
    s
}

/// Inserts segments right after SOI.
fn splice(jpeg: &[u8], segs: &[Vec<u8>]) -> Vec<u8> {
    let mut v = jpeg[..2].to_vec();
    for s in segs {
        v.extend_from_slice(s);
    }
    v.extend_from_slice(&jpeg[2..]);
    v
}

/// ISO 21496-1 payload, either with a common denominator or per-value denominators.
fn iso_payload(p: &GainMapParams, multichannel: bool, common: bool) -> Vec<u8> {
    let mut b = vec![0, 0, 0, 0];
    let flags = if multichannel { 0x80 } else { 0 }
        | if common { 0x08 } else { 0 }
        | if p.base_is_hdr { 0x04 } else { 0 };
    b.push(flags);
    let d: u32 = 10_000;
    let u = |b: &mut Vec<u8>, v: f32| {
        b.extend_from_slice(&((v * d as f32).round() as u32).to_be_bytes());
        if !common {
            b.extend_from_slice(&d.to_be_bytes());
        }
    };
    let i = |b: &mut Vec<u8>, v: f32| {
        b.extend_from_slice(&((v * d as f32).round() as i32).to_be_bytes());
        if !common {
            b.extend_from_slice(&d.to_be_bytes());
        }
    };
    if common {
        b.extend_from_slice(&d.to_be_bytes());
    }
    u(&mut b, p.base_headroom);
    u(&mut b, p.alt_headroom);
    for c in 0..if multichannel { 3 } else { 1 } {
        i(&mut b, p.gain_min[c]);
        i(&mut b, p.gain_max[c]);
        u(&mut b, p.gamma[c]);
        i(&mut b, p.base_offset[c]);
        i(&mut b, p.alt_offset[c]);
    }
    b
}

fn hdrgm_xmp(extra: &str, map_len: Option<usize>) -> Vec<u8> {
    let container = map_len.map_or(String::new(), |n| {
        format!(
            r#"<Container:Directory><rdf:Seq>
<rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="Primary" Item:Mime="image/jpeg"/></rdf:li>
<rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="GainMap" Item:Mime="image/jpeg" Item:Length="{n}"/></rdf:li>
</rdf:Seq></Container:Directory>"#
        )
    });
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/" hdrgm:Version="1.0" {extra}>{container}</rdf:Description>
</rdf:RDF></x:xmpmeta>"#
    )
    .into_bytes()
}

/// MPF APP2 segment with two images; sizes/offsets patched by [`patch_mpf`].
fn mpf_segment() -> Vec<u8> {
    let mut m = b"MM\0*".to_vec();
    m.extend_from_slice(&8u32.to_be_bytes());
    m.extend_from_slice(&3u16.to_be_bytes());
    let entry = |m: &mut Vec<u8>, tag: u16, ty: u16, cnt: u32, val: [u8; 4]| {
        m.extend_from_slice(&tag.to_be_bytes());
        m.extend_from_slice(&ty.to_be_bytes());
        m.extend_from_slice(&cnt.to_be_bytes());
        m.extend_from_slice(&val);
    };
    entry(&mut m, 0xB000, 7, 4, *b"0100");
    entry(&mut m, 0xB001, 4, 1, 2u32.to_be_bytes());
    entry(&mut m, 0xB002, 7, 32, 50u32.to_be_bytes());
    m.extend_from_slice(&0u32.to_be_bytes());
    assert_eq!(m.len(), 50);
    m.extend_from_slice(&[0; 32]);
    segment(0xE2, b"MPF\0", &m)
}

fn patch_mpf(file: &mut [u8], primary_len: usize, map_len: usize, bad_offset: bool) {
    let p = file.windows(8).position(|w| w == b"MPF\0MM\0*").unwrap();
    let tiff = p + 4;
    let at = tiff + 50;
    let put = |f: &mut [u8], o: usize, v: u32| f[o..o + 4].copy_from_slice(&v.to_be_bytes());
    put(file, at + 4, primary_len as u32);
    put(file, at + 16 + 4, map_len as u32);
    let off = if bad_offset {
        7
    } else {
        (primary_len - tiff) as u32
    };
    put(file, at + 16 + 8, off);
}

#[derive(Clone, Copy)]
struct Opts {
    mpf: bool,
    container: bool,
    iso: bool,
    xmp_in_map: bool,
    bad_mpf: bool,
    padding: usize,
}

const ULTRA_HDR: Opts = Opts {
    mpf: true,
    container: true,
    iso: true,
    xmp_in_map: true,
    bad_mpf: false,
    padding: 0,
};

fn params() -> GainMapParams {
    GainMapParams {
        gain_min: [0.0, 0.125, 0.25],
        gain_max: [2.5, 2.0, 1.5],
        gamma: [1.0, 1.25, 1.0],
        base_offset: [1.0 / 64.0; 3],
        alt_offset: [1.0 / 64.0; 3],
        base_headroom: 0.0,
        alt_headroom: 2.5,
        base_is_hdr: false,
        ..Default::default()
    }
}

fn xmp_attrs(p: &GainMapParams) -> String {
    format!(
        r#"hdrgm:GainMapMin="{}" hdrgm:HDRCapacityMin="{}" hdrgm:HDRCapacityMax="{}""#,
        p.gain_min[0], p.base_headroom, p.alt_headroom
    )
}

/// An Ultra HDR-style file: primary (XMP container, ISO version stub, MPF) + gain map
/// (ISO metadata and/or hdrgm XMP).
fn ultra_hdr(o: Opts, p: &GainMapParams) -> Vec<u8> {
    let mut map_segs = Vec::new();
    if o.iso {
        map_segs.push(segment(0xE2, ISO_SIG, &iso_payload(p, true, true)));
    }
    if o.xmp_in_map {
        let extra = format!(
            r#"{} hdrgm:GainMapMax="{}" hdrgm:Gamma="{}" hdrgm:OffsetSDR="{}" hdrgm:OffsetHDR="{}""#,
            xmp_attrs(p),
            p.gain_max[0],
            p.gamma[0],
            p.base_offset[0],
            p.alt_offset[0]
        );
        map_segs.push(segment(0xE1, XMP_SIG, &hdrgm_xmp(&extra, None)));
    }
    let map = splice(&map_jpeg(48, 32), &map_segs);
    let mut segs = vec![segment(
        0xE1,
        XMP_SIG,
        &hdrgm_xmp("", o.container.then_some(map.len())),
    )];
    if o.iso {
        segs.push(segment(0xE2, ISO_SIG, &[0, 0, 0, 0]));
    }
    if o.mpf {
        segs.push(mpf_segment());
    }
    let mut file = splice(&base_jpeg(96, 64), &segs);
    let primary_len = file.len() + o.padding;
    if o.mpf {
        patch_mpf(&mut file, primary_len, map.len(), o.bad_mpf);
    }
    file.extend(std::iter::repeat_n(0u8, o.padding));
    file.extend_from_slice(&map);
    file
}

fn assert_params(got: &GainMapParams, want: &GainMapParams, channels: usize) {
    let eq = |a: f32, b: f32| (a - b).abs() < 1e-3;
    for c in 0..3 {
        let w = if channels == 1 { 0 } else { c };
        assert!(eq(got.gain_min[c], want.gain_min[w]), "{got:?}");
        assert!(eq(got.gain_max[c], want.gain_max[w]), "{got:?}");
        assert!(eq(got.gamma[c], want.gamma[w]), "{got:?}");
        assert!(eq(got.base_offset[c], want.base_offset[w]), "{got:?}");
        assert!(eq(got.alt_offset[c], want.alt_offset[w]), "{got:?}");
    }
    assert!(eq(got.base_headroom, want.base_headroom));
    assert!(eq(got.alt_headroom, want.alt_headroom));
    assert_eq!(got.base_is_hdr, want.base_is_hdr);
}

#[test]
fn iso21496_both_layouts() {
    let mut p = params();
    for (multi, common, backward) in [
        (true, true, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        p.base_is_hdr = backward;
        let b = iso_payload(&p, multi, common);
        let got = parse_iso21496(&b).expect("parses");
        // The backward flag (libultrahdr) keeps SDR, HDR order: normalised to base first.
        let mut want = p;
        if backward {
            want.base_headroom = p.alt_headroom;
            want.alt_headroom = p.base_headroom;
            want.base_offset = p.alt_offset;
            want.alt_offset = p.base_offset;
        }
        assert_params(&got, &want, if multi { 3 } else { 1 });
        // Truncated payloads and the version-only stub are rejected, never panic.
        for n in 0..b.len() {
            assert!(parse_iso21496(&b[..n]).is_none(), "len {n}");
        }
    }
    // ISO 21496-1 proper: an HDR base is a base headroom above the alternate's, no flag.
    let mut q = params();
    q.base_headroom = 2.5;
    q.alt_headroom = 0.0;
    let got = parse_iso21496(&iso_payload(&q, false, false)).unwrap();
    assert!(got.base_is_hdr);
    assert_eq!((got.base_headroom, got.alt_headroom), (2.5, 0.0));
    assert!(got.weight(1.0) == -1.0 && got.weight(8.0) == 0.0);
    p.base_is_hdr = false;
    // Unknown minimum version.
    let mut b = iso_payload(&p, true, true);
    b[1] = 1;
    assert!(parse_iso21496(&b).is_none());
    // Zero denominator.
    let mut b = iso_payload(&p, false, true);
    b[5..9].copy_from_slice(&0u32.to_be_bytes());
    assert!(parse_iso21496(&b).is_none());
}

#[test]
fn hdrgm_xmp_forms() {
    // Attributes, single values.
    let x = hdrgm_xmp(
        r#"hdrgm:GainMapMin="-0.5" hdrgm:GainMapMax="3" hdrgm:Gamma="1.5" hdrgm:OffsetSDR="0" hdrgm:OffsetHDR="0.01" hdrgm:HDRCapacityMin="0.25" hdrgm:HDRCapacityMax="3""#,
        None,
    );
    let p = parse_hdrgm_xmp(&x).unwrap();
    assert_eq!(p.gain_min, [-0.5; 3]);
    assert_eq!(p.gain_max, [3.0; 3]);
    assert_eq!(p.gamma, [1.5; 3]);
    assert_eq!(p.base_offset, [0.0; 3]);
    assert_eq!(p.alt_offset, [0.01; 3]);
    assert_eq!((p.base_headroom, p.alt_headroom), (0.25, 3.0));
    assert!(!p.base_is_hdr);

    // Elements with per-channel rdf:Seq; capacity max defaults to the largest gain.
    let x = br#"<rdf:Description xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
<hdrgm:Version>1.0</hdrgm:Version>
<hdrgm:GainMapMax><rdf:Seq><rdf:li>1</rdf:li><rdf:li>2</rdf:li><rdf:li>3.5</rdf:li></rdf:Seq></hdrgm:GainMapMax>
<hdrgm:GainMapMaxX>9</hdrgm:GainMapMaxX>
<hdrgm:Gamma><rdf:Seq><rdf:li> 2 </rdf:li></rdf:Seq></hdrgm:Gamma>
</rdf:Description>"#;
    let p = parse_hdrgm_xmp(x).unwrap();
    assert_eq!(p.gain_max, [1.0, 2.0, 3.5]);
    assert_eq!(p.gamma, [2.0; 3]);
    assert_eq!(p.gain_min, [0.0; 3]);
    assert_eq!(p.alt_headroom, 3.5);

    // HDR base: offsets and headrooms swap roles.
    let x = hdrgm_xmp(
        r#"hdrgm:GainMapMax="2" hdrgm:OffsetSDR="0.1" hdrgm:OffsetHDR="0.2" hdrgm:HDRCapacityMin="0" hdrgm:HDRCapacityMax="2" hdrgm:BaseRenditionIsHDR="True""#,
        None,
    );
    let p = parse_hdrgm_xmp(&x).unwrap();
    assert!(p.base_is_hdr);
    assert_eq!((p.base_offset[0], p.alt_offset[0]), (0.2, 0.1));
    assert_eq!((p.base_headroom, p.alt_headroom), (2.0, 0.0));

    // No version / no max: not gain-map metadata.
    assert!(parse_hdrgm_xmp(br#"<x hdrgm:GainMapMax="2"/>"#).is_none());
    assert!(parse_hdrgm_xmp(&hdrgm_xmp("", None)).is_none());
    // Garbage values.
    assert!(parse_hdrgm_xmp(&hdrgm_xmp(r#"hdrgm:GainMapMax="lots""#, None)).is_none());
}

#[test]
fn locates_and_decodes_the_map() {
    let p = params();
    let cancel = CancelToken::new();
    let cases = [
        ("ultra hdr", ULTRA_HDR, "ISO 21496-1"),
        (
            "xmp only (Adobe)",
            Opts {
                iso: false,
                ..ULTRA_HDR
            },
            "Ultra HDR",
        ),
        (
            "iso only, no container",
            Opts {
                xmp_in_map: false,
                container: false,
                ..ULTRA_HDR
            },
            "ISO 21496-1",
        ),
        (
            "container length only",
            Opts {
                mpf: false,
                ..ULTRA_HDR
            },
            "ISO 21496-1",
        ),
        (
            "concatenated with padding",
            Opts {
                mpf: false,
                container: false,
                padding: 100,
                ..ULTRA_HDR
            },
            "ISO 21496-1",
        ),
        (
            "broken MPF offset",
            Opts {
                bad_mpf: true,
                ..ULTRA_HDR
            },
            "ISO 21496-1",
        ),
    ];
    for (name, o, source) in cases {
        let file = ultra_hdr(o, &p);
        assert!(jpeg_has_gain_map(&file), "{name}");
        let gm = jpeg_gain_map(&file, &cancel).unwrap_or_else(|| panic!("{name}: no map"));
        assert_eq!(gm.source, source, "{name}");
        // XMP carries single values only; ISO all three channels.
        assert_params(
            &gm.params,
            &p,
            if source.starts_with("ISO") { 3 } else { 1 },
        );
        assert_eq!((gm.map.width, gm.map.height), (48, 32), "{name}");
        assert_eq!(gm.map.layout, PixelLayout::Rgba);
        let row = &gm.map.planes[0].data[..48 * 4];
        for x in [0usize, 12, 24, 47] {
            let want = (x * 255 / 47) as i32;
            let px = &row[x * 4..x * 4 + 4];
            for &v in &px[..3] {
                assert!(
                    (v as i32 - want).abs() <= 6,
                    "{name} x {x}: {px:?} vs {want}"
                );
            }
            assert_eq!(px[3], 255);
        }
    }
}

#[test]
fn plain_jpeg_and_plain_mpf_have_no_map() {
    let cancel = CancelToken::new();
    let plain = base_jpeg(32, 16);
    assert!(!jpeg_has_gain_map(&plain));
    assert!(jpeg_gain_map(&plain, &cancel).is_none());
    // An MPF pair (e.g. a stereo/preview image) without gain-map metadata.
    let second = base_jpeg(16, 8);
    let mut file = splice(&plain, &[mpf_segment()]);
    let primary_len = file.len();
    patch_mpf(&mut file, primary_len, second.len(), false);
    file.extend_from_slice(&second);
    assert!(!jpeg_has_gain_map(&file));
    assert!(jpeg_gain_map(&file, &cancel).is_none());
}

#[test]
fn insane_metadata_is_ignored() {
    let mut p = params();
    p.gain_max = [1000.0; 3];
    let file = ultra_hdr(ULTRA_HDR, &p);
    assert!(jpeg_gain_map(&file, &CancelToken::new()).is_none());
}

#[test]
fn truncated_and_corrupt_files_do_not_panic() {
    let file = ultra_hdr(ULTRA_HDR, &params());
    let cancel = CancelToken::new();
    let step = (file.len() / 300).max(1);
    for n in (0..file.len()).step_by(step) {
        let _ = jpeg_has_gain_map(&file[..n]);
        let _ = jpeg_gain_map(&file[..n], &cancel);
    }
    let mut x = 0x2545_f491_u32;
    for _ in 0..200 {
        let mut f = file.clone();
        for _ in 0..4 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % f.len();
            f[i] = (x >> 8) as u8;
        }
        let _ = jpeg_has_gain_map(&f);
        let _ = jpeg_gain_map(&f, &cancel);
    }
    // Cancelled before starting.
    cancel.cancel();
    assert!(jpeg_gain_map(&file, &cancel).is_none());
}

#[test]
fn session_exposes_the_gain_map() {
    let file = ultra_hdr(ULTRA_HDR, &params());
    let reg = Registry::new();
    let s = reg
        .open(Arc::new(Source::from_vec("uhdr.jpg", file)))
        .unwrap();
    assert!(s.has_gain_map());
    let gm = s.gain_map(&CancelToken::new()).unwrap();
    assert_eq!(gm.map.width, 48);

    let s = reg
        .open(Arc::new(Source::from_vec("plain.jpg", base_jpeg(16, 16))))
        .unwrap();
    assert!(!s.has_gain_map());
    assert!(s.gain_map(&CancelToken::new()).is_none());
}

fn apple_xmp(extra: &str) -> Vec<u8> {
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:HDRGainMap="http://ns.apple.com/HDRGainMap/1.0/" xmlns:apdi="http://ns.apple.com/pixeldatainfo/1.0/" apdi:AuxiliaryImageType="urn:com:apple:photo:2020:aux:hdrgainmap" HDRGainMap:HDRGainMapVersion="131072" {extra}/>
</rdf:RDF></x:xmpmeta>"#
    )
    .into_bytes()
}

#[test]
fn apple_headroom_sources() {
    let x = apple_xmp(r#"HDRGainMap:HDRGainMapHeadroom="4.532783""#);
    assert_eq!(apple_headroom(Some(&x), None), Some(4.532783));
    let x = br#"<HDRGainMap:HDRGainMapHeadroom> 2.5 </HDRGainMap:HDRGainMapHeadroom>"#;
    assert_eq!(apple_headroom(Some(x), None), Some(2.5));
    // No headroom anywhere, or none above SDR: no gain map.
    assert_eq!(apple_headroom(Some(&apple_xmp("")), None), None);
    let x = apple_xmp(r#"HDRGainMap:HDRGainMapHeadroom="1""#);
    assert_eq!(apple_headroom(Some(&x), None), None);
    assert_eq!(apple_headroom(None, Some(b"II*\0garbage")), None);
}

/// Apple's reconstruction, `sdr · (1 + (H − 1) · lin(g))`, through the shared ISO 21496-1
/// model at full weight, and partially applied in between.
#[test]
fn apple_curve_through_the_iso_model() {
    let data: Vec<u8> = (0..=255u8).flat_map(|v| [v, v, v, 255]).collect();
    let map = Raster {
        width: 256,
        height: 1,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![specular_core::RasterPlane {
            width: 256,
            height: 1,
            stride: 256 * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    };
    let h = 4.0f32;
    let gm = apple_gain_map(&map, h).unwrap();
    assert_eq!(gm.source, "Apple");
    let p = gm.params;
    assert!(!p.base_is_hdr && p.space == Primaries::Bt709);
    assert_eq!(
        (p.weight(1.0), p.weight(2.0), p.weight(8.0)),
        (0.0, 0.5, 1.0)
    );
    assert!((p.peak(1.0, 1.0) - h).abs() < 1e-4);
    let base = 0.5;
    for v in 0..=255usize {
        let g = gm.map.planes[0].data[v * 4] as f32 / 255.0;
        let want = base * (1.0 + (h - 1.0) * srgb_to_linear(v as f32 / 255.0));
        let got = p.apply(0, base, g, 1.0);
        assert!(
            (got / want - 1.0).abs() < 0.006,
            "level {v}: {got} vs {want}"
        );
        // Half weight: the geometric midpoint between the renditions.
        let half = p.apply(0, base, g, 0.5);
        assert!(
            (half / (base * want).sqrt() - 1.0).abs() < 0.006,
            "level {v}"
        );
    }
    // Not usable: no headroom, or not RGBA8.
    assert!(apple_gain_map(&map, 1.0).is_none());
}

#[test]
fn apple_jpeg() {
    let map = splice(
        &map_jpeg(48, 32),
        &[segment(
            0xE1,
            XMP_SIG,
            &apple_xmp(r#"HDRGainMap:HDRGainMapHeadroom="3.5""#),
        )],
    );
    let mut file = splice(&base_jpeg(96, 64), &[mpf_segment()]);
    let primary_len = file.len();
    patch_mpf(&mut file, primary_len, map.len(), false);
    file.extend_from_slice(&map);
    assert!(jpeg_has_gain_map(&file));
    let gm = jpeg_gain_map(&file, &CancelToken::new()).unwrap();
    assert_eq!(gm.source, "Apple");
    assert!((gm.params.alt_headroom - 3.5f32.log2()).abs() < 1e-5);
    assert_eq!((gm.map.width, gm.map.height), (48, 32));
    // Without the MPF index a trailing Apple map is not announced (plain MPF previews).
    let mut plain = base_jpeg(96, 64);
    plain.extend_from_slice(&map_jpeg(48, 32));
    assert!(!jpeg_has_gain_map(&plain));
}

#[test]
fn tmap_item_body() {
    let mut p = params();
    let multi = |p: &GainMapParams, flags: u8| {
        let mut b = vec![0];
        b.extend(iso_payload(p, true, false));
        b[5] |= flags;
        b
    };
    let p3 = Some(Primaries::DisplayP3);
    let bt2020 = Some(Primaries::Bt2020);
    // Per-channel metadata: the alternate's primaries, or the base's with flag 0x40.
    assert_eq!(
        tmap_params(&multi(&p, 0), p3, bt2020).unwrap().space,
        Primaries::Bt2020
    );
    assert_eq!(
        tmap_params(&multi(&p, 0x40), p3, bt2020).unwrap().space,
        Primaries::DisplayP3
    );
    assert_eq!(
        tmap_params(&multi(&p, 0), p3, None).unwrap().space,
        Primaries::Bt709
    );
    // Single channel: equal gains commute with the primaries.
    let mut b = vec![0];
    b.extend(iso_payload(&p, false, false));
    assert_eq!(tmap_params(&b, p3, bt2020).unwrap().space, Primaries::Bt709);
    // Unknown item version; insane values.
    b[0] = 1;
    assert!(tmap_params(&b, p3, bt2020).is_none());
    p.gain_max = [40.0; 3];
    assert!(tmap_params(&multi(&p, 0), p3, bt2020).is_none());
}

/// The metadata of a JPEG XL `jhgm` bundle: plain ISO 21496-1, or (leniently) with an AVIF
/// `tmap`-style version byte in front.
#[test]
fn jhgm_metadata() {
    let mut p = params();
    let p3 = Some(Primaries::DisplayP3);
    let bt2020 = Some(Primaries::Bt2020);
    for common in [false, true] {
        let plain = iso_payload(&p, true, common);
        let a = jhgm_params(&plain, p3, bt2020).unwrap();
        assert_eq!(a.space, Primaries::Bt2020);
        assert!((a.gain_max[1] - p.gain_max[1]).abs() < 1e-4);
        let mut flagged = plain.clone();
        flagged[4] |= 0x40;
        assert_eq!(
            jhgm_params(&flagged, p3, bt2020).unwrap().space,
            Primaries::DisplayP3
        );
        let mut tmap = vec![0];
        tmap.extend_from_slice(&flagged);
        assert_eq!(
            jhgm_params(&tmap, p3, bt2020),
            jhgm_params(&flagged, p3, bt2020)
        );
        // Trailing bytes after plain metadata (a later minor version) are ignored.
        let mut longer = plain.clone();
        longer.extend_from_slice(&[0; 7]);
        assert_eq!(jhgm_params(&longer, p3, bt2020), Some(a));
        // Cut short: never the original parameters (a cut can happen to read as a complete
        // shorter payload in the prefixed form; whatever it gives has passed `is_sane`).
        for n in 0..plain.len() {
            let r = jhgm_params(&plain[..n], p3, bt2020);
            assert!(r.is_none_or(|r| r != a && r.is_sane()), "{n}");
        }
    }
    // Unknown minimum version; insane values.
    let mut b = iso_payload(&p, false, true);
    b[1] = 1;
    assert!(jhgm_params(&b, None, None).is_none());
    p.gamma = [0.0; 3];
    assert!(jhgm_params(&iso_payload(&p, false, true), None, None).is_none());
}

/// Prints `ORACLE<TAB>GAINMAP/<ext><TAB>file<TAB>pass|fail` when dropped (fail while a check
/// panics), one line per corpus file for tools/ci/oracle-pass-rates.sh. Absent files print
/// `skip` through [`skipped`].
struct Verdict(&'static str);

impl Drop for Verdict {
    fn drop(&mut self) {
        let status = if std::thread::panicking() {
            "fail"
        } else {
            "pass"
        };
        oracle_line(self.0, status);
    }
}

fn oracle_line(name: &str, status: &str) {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_uppercase();
    println!("ORACLE\tGAINMAP/{ext}\t{name}\t{status}");
}

fn skipped(name: &str) {
    eprintln!("{name}: not in the corpus, skipped");
    oracle_line(name, "skip");
}

/// A file of the conformance corpus: `SPECULAR_CONFORMANCE_CORPUS` when set (as
/// tools/corpus/fetch-conformance.sh takes it), else tools/corpus/conformance.
fn corpus(rel: &str) -> Option<std::path::PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        });
    let p = root.join(rel);
    p.is_file().then_some(p)
}

fn open_corpus(rel: &str) -> Option<Arc<dyn specular_decode::Session>> {
    let p = corpus(rel)?;
    Some(
        Registry::new()
            .open(Arc::new(Source::open(&p).unwrap()))
            .unwrap(),
    )
}

/// Apple JPEGs from libavif's / libultrahdr's test data: the old map (version 1) takes its
/// headroom from the maker notes, the new one (version 2) from its own XMP.
#[test]
fn corpus_apple_jpegs() {
    for (name, stops) in [
        ("libavif/tests/data/apple_gainmap_old.jpg", 3.0f32),
        (
            "libavif/tests/data/apple_gainmap_new.jpg",
            4.532783f32.log2(),
        ),
    ] {
        let Some(s) = open_corpus(name) else {
            skipped(name);
            continue;
        };
        let _v = Verdict(name);
        assert!(s.has_gain_map(), "{name}");
        let gm = s.gain_map(&CancelToken::new()).expect(name);
        assert_eq!(gm.source, "Apple", "{name}");
        assert!(
            (gm.params.alt_headroom - stops).abs() < 0.01,
            "{name}: {:?}",
            gm.params
        );
    }
}

/// An iPhone HEIC with Apple's `hdrgainmap` auxiliary (no `tmap`): headroom from the map's
/// XMP or the maker notes.
#[test]
fn corpus_iphone_heic_apple_aux() {
    let name = "apple-hdr-heic/tests/data/hdr-sample.heic";
    let Some(s) = open_corpus(name) else {
        skipped(name);
        return;
    };
    let _v = Verdict(name);
    assert!(s.has_gain_map());
    assert!(
        s.info().coding.contains("gain map (Apple)"),
        "{}",
        s.info().coding
    );
    let gm = s.gain_map(&CancelToken::new()).unwrap();
    assert_eq!(gm.source, "Apple");
    let p = gm.params;
    eprintln!(
        "{name}: headroom 2^{:.3}, map {}×{}",
        p.alt_headroom, gm.map.width, gm.map.height
    );
    assert!(p.alt_headroom > 0.0 && p.alt_headroom <= 8.0, "{p:?}");
    assert!(!p.base_is_hdr);
    let (w, h) = (s.info().width as f32, s.info().height as f32);
    let (mw, mh) = (gm.map.width as f32, gm.map.height as f32);
    assert!((mw / mh - w / h).abs() < 0.02, "{w}x{h} vs map {mw}x{mh}");
}

/// An iPhone HEIC (iOS 18) with an ISO 21496-1 `tmap` whose map is a grid.
#[test]
fn corpus_iphone_heic_tmap() {
    let name = "zune-image/crates/zune-heic/fuzz-samples/IMG_4862.HEIC";
    let Some(s) = open_corpus(name) else {
        skipped(name);
        return;
    };
    let _v = Verdict(name);
    assert!(s.has_gain_map());
    assert!(
        s.info().coding.contains("gain map (ISO 21496-1)"),
        "{}",
        s.info().coding
    );
    let gm = s.gain_map(&CancelToken::new()).unwrap();
    assert_eq!(gm.source, "ISO 21496-1");
    let p = gm.params;
    assert!((p.alt_headroom - 2.296875).abs() < 1e-4, "{p:?}");
    assert!(!p.base_is_hdr && p.space == Primaries::Bt709);
    // The map covers the image with the same aspect ratio (a quarter or so of its size).
    let (w, h) = (s.info().width as f32, s.info().height as f32);
    let (mw, mh) = (gm.map.width as f32, gm.map.height as f32);
    eprintln!("{name}: {w}x{h}, map {mw}x{mh}");
    assert!((mw / mh - w / h).abs() < 0.02, "{w}x{h} vs map {mw}x{mh}");
    // Not flat: a real map spans a range of gains.
    let d = &gm.map.planes[0].data;
    let (lo, hi) = d
        .chunks_exact(4)
        .fold((255u8, 0u8), |(lo, hi), px| (lo.min(px[0]), hi.max(px[0])));
    assert!(hi - lo > 64, "{lo}..{hi}");
}

/// libavif's gain-map AVIFs. The SDR-base Seine image also ships as a JPEG carrying the same
/// map and metadata, which makes the two paths each other's oracle; unsupported metadata
/// versions get no map. (HDR bases: `corpus_avif_hdr_base_gain_map`.)
#[test]
fn corpus_avif_gain_maps() {
    let dir = "libavif/tests/data/";
    let Some(avif) = open_corpus(&format!("{dir}seine_sdr_gainmap_srgb.avif")) else {
        skipped("libavif/tests/data/seine_sdr_gainmap_srgb.avif");
        return;
    };
    let _v = Verdict("libavif/tests/data/seine_sdr_gainmap_srgb.avif");
    let cancel = CancelToken::new();
    assert!(avif.has_gain_map());
    assert!(avif.info().coding.contains("gain map (ISO 21496-1)"));
    let a = avif.gain_map(&cancel).unwrap();
    assert_eq!(a.source, "ISO 21496-1");
    assert!(
        !a.params.base_is_hdr && a.params.alt_headroom > 0.0,
        "{:?}",
        a.params
    );
    if let Some(jpeg) = open_corpus(&format!("{dir}seine_sdr_gainmap_srgb.jpg")) {
        let j = jpeg.gain_map(&cancel).unwrap();
        let (pa, pj) = (a.params, j.params);
        for c in 0..3 {
            assert!(
                (pa.gain_max[c] - pj.gain_max[c]).abs() < 1e-3,
                "{pa:?} vs {pj:?}"
            );
            assert!(
                (pa.gain_min[c] - pj.gain_min[c]).abs() < 1e-3,
                "{pa:?} vs {pj:?}"
            );
            assert!((pa.gamma[c] - pj.gamma[c]).abs() < 1e-3, "{pa:?} vs {pj:?}");
        }
        assert!((pa.alt_headroom - pj.alt_headroom).abs() < 1e-3);
        assert_eq!((a.map.width, a.map.height), (j.map.width, j.map.height));
        // Same map through AV1 and JPEG coding: close, not identical.
        let (da, dj) = (&a.map.planes[0].data, &j.map.planes[0].data);
        let mean = da
            .iter()
            .zip(dj)
            .map(|(&x, &y)| f64::from(x.abs_diff(y)))
            .sum::<f64>()
            / da.len() as f64;
        assert!(mean < 3.0, "mean |avif − jpeg| = {mean}");
    }
    // A map that is a grid while the base is not, and the other way round (the map need not
    // share the base's aspect ratio: it is stretched over it). These bases are PQ HDR.
    for name in [
        "color_nogrid_alpha_nogrid_gainmap_grid.avif",
        "color_grid_gainmap_different_grid.avif",
    ] {
        let path = corpus(&format!("{dir}{name}")).unwrap();
        let f = specular_avif::AvifFile::open(std::fs::read(path).unwrap()).unwrap();
        let g = f.gain_map().expect(name);
        let p = tmap_params(&g.metadata, None, None).expect(name);
        assert!(p.base_is_hdr, "{name}: {p:?}");
        let d = f.decode_gain_map(true, &|| false).unwrap();
        assert_eq!((d.width, d.height), (g.width, g.height), "{name}");
        let (lo, hi) = d
            .data
            .iter()
            .fold((255u8, 0u8), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi > lo, "{name}: flat map");
    }
    for name in [
        "unsupported_gainmap_version.avif",
        "unsupported_gainmap_minimum_version.avif",
        "seine_sdr_gainmap_gammazero.avif",
    ] {
        let s = open_corpus(&format!("{dir}{name}")).unwrap();
        assert!(!s.has_gain_map(), "{name}: {}", s.info().coding);
        assert!(s.info().coding.contains("gain map (not applied)"), "{name}");
        assert!(s.gain_map(&cancel).is_none(), "{name}");
    }
}

/// libavif's Seine pair: the same picture and map once on a PQ base (headroom 1.3 stops,
/// alternate SDR) and once on an SDR base. The HDR base decodes wide (16-bit PQ, no 8-bit
/// tone mapping), its map comes with a negative weight for an SDR display, and the full
/// map down gives the SDR picture (the other file's base) back.
#[test]
fn corpus_avif_hdr_base_gain_map() {
    let dir = "libavif/tests/data/";
    let (Some(hdr), Some(sdr)) = (
        open_corpus(&format!("{dir}seine_hdr_gainmap_srgb.avif")),
        open_corpus(&format!("{dir}seine_sdr_gainmap_srgb.avif")),
    ) else {
        skipped("libavif/tests/data/seine_hdr_gainmap_srgb.avif");
        return;
    };
    let _v = Verdict("libavif/tests/data/seine_hdr_gainmap_srgb.avif");
    let cancel = CancelToken::new();
    let coding = &hdr.info().coding;
    assert!(hdr.has_gain_map(), "{coding}");
    assert!(
        coding.contains("PQ HDR") && !coding.contains("tone mapped"),
        "{coding}"
    );
    assert!(
        coding.contains("gain map (ISO 21496-1, HDR base)"),
        "{coding}"
    );
    let gm = hdr.gain_map(&cancel).unwrap();
    let p = gm.params;
    assert!(p.base_is_hdr, "{p:?}");
    assert!((p.base_headroom - 1.3).abs() < 1e-3 && p.alt_headroom.abs() < 1e-3);
    assert_eq!(p.weight(1.0), -1.0);
    assert_eq!(p.weight(4.0), 0.0);
    assert!((p.weight(0.65f32.exp2()) + 0.5).abs() < 1e-4);
    // The SDR file reads the same map from the other side.
    let q = sdr.gain_map(&cancel).unwrap().params;
    assert!(
        !q.base_is_hdr && (q.alt_headroom - 1.3).abs() < 1e-3,
        "{q:?}"
    );
    for c in 0..3 {
        assert!((p.gain_max[c] - q.gain_max[c]).abs() < 1e-4);
        assert!((p.gain_min[c] - q.gain_min[c]).abs() < 1e-4);
    }
    let sdr_px = sdr.decode(&DecodeRequest::full(1), &cancel).unwrap();
    assert_eq!(sdr_px.layout, PixelLayout::Rgb);
    let (w, h) = (sdr_px.width as usize, sdr_px.height as usize);
    for name in [
        "seine_hdr_gainmap_srgb.avif",
        "seine_hdr_gainmap_small_srgb.avif",
    ] {
        let s = open_corpus(&format!("{dir}{name}")).unwrap();
        let base = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        let PixelLayout::Rgba16(enc) = base.layout else {
            panic!("{name}: {:?}", base.layout);
        };
        assert_eq!(enc.transfer, specular_core::Transfer::Pq);
        assert_eq!(enc.primaries, Primaries::Bt709);
        assert_eq!((base.width as usize, base.height as usize), (w, h));
        // Not clipped to SDR: some pixels are above reference white.
        let bright = base.planes[0]
            .data
            .chunks_exact(8)
            .filter(|px| {
                let u = |i: usize| u16::from_ne_bytes([px[2 * i], px[2 * i + 1]]);
                specular_core::hdr::decode16(enc, [u(0), u(1), u(2)])[1] > 1.2
            })
            .count();
        assert!(bright > w * h / 100, "{name}: {bright} pixels above white");
        let gm = s.gain_map(&cancel).unwrap();
        let peak = specular_core::hdr::peak_from_nits(None);
        let r = specular_core::hdr::sdr_rendition(&base, &gm, peak).unwrap();
        let out = &r.planes[0].data;
        let (mut sum, mut worst) = (0f64, 0u8);
        let (mut bias, mut far) = ([0f64; 3], 0usize);
        for i in 0..w * h {
            for c in 0..3 {
                let d = out[i * 4 + c].abs_diff(sdr_px.planes[c].data[i]);
                sum += f64::from(d);
                worst = worst.max(d);
                bias[c] += f64::from(out[i * 4 + c]) - f64::from(sdr_px.planes[c].data[i]);
                far += usize::from(d > 40);
            }
        }
        let mean = sum / (w * h * 3) as f64;
        let bias = bias.map(|b| b / (w * h) as f64);
        eprintln!(
            "{name}: mean |rendition − SDR| = {mean:.2}, bias {bias:.2?}, worst {worst} \
             ({far} samples off by more than 40)"
        );
        // Both files are lossy AV1 and the map is a quarter-size 8-bit picture: close, with
        // a few single-pixel highlights off where the map has an edge.
        assert!(mean < 5.0, "{name}: mean {mean}");
        assert!(bias.iter().all(|b| b.abs() < 1.5), "{name}: bias {bias:?}");
        assert!(far * 1000 < w * h * 3, "{name}: {far} samples far off");
        // The base alone, generically tone mapped, is further off: the map matters.
        let plain = base.planes[0]
            .data
            .chunks_exact(8)
            .enumerate()
            .map(|(i, px)| {
                let t = specular_core::hdr::wide_to_srgb8(base.layout, px, peak);
                (0..3)
                    .map(|c| f64::from(t[c].abs_diff(sdr_px.planes[c].data[i])))
                    .sum::<f64>()
            })
            .sum::<f64>()
            / (w * h * 3) as f64;
        eprintln!("{name}: mean |tone mapped base − SDR| = {plain:.2}");
        assert!(plain > mean, "{name}: {plain} vs {mean}");
    }
}

#[test]
#[ignore = "timing; run with --release --ignored --nocapture"]
fn bench_gain_map_decode() {
    // A 12 MP-ish primary is not needed: the cost is the map (quarter resolution).
    let (w, h) = (1000u16, 750u16);
    let px: Vec<u8> = (0..w as usize * h as usize)
        .map(|i| (i % 251) as u8)
        .collect();
    let map = splice(
        &encode(w, h, &px, ColorType::Luma),
        &[segment(0xE2, ISO_SIG, &iso_payload(&params(), true, true))],
    );
    let mut file = splice(&base_jpeg(64, 64), &[segment(0xE2, ISO_SIG, &[0, 0, 0, 0])]);
    file.extend_from_slice(&map);
    let cancel = CancelToken::new();
    let t = std::time::Instant::now();
    let n = 20;
    for _ in 0..n {
        assert!(jpeg_gain_map(&file, &cancel).is_some());
    }
    eprintln!(
        "gain map {w}x{h}: {:.2} ms per decode (incl. RGBA expansion)",
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    );
    let t = std::time::Instant::now();
    for _ in 0..1000 {
        assert!(jpeg_has_gain_map(&file));
    }
    eprintln!("has_gain_map: {:.2} us", t.elapsed().as_secs_f64() * 1e3);
}
