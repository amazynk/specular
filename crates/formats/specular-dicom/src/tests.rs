//! Synthetic files: every encoding, sequences, encapsulation, photometrics and display
//! mapping, plus a mutation sweep for robustness.

use super::*;

/// Writes data elements in one encoding.
#[derive(Clone)]
pub(crate) struct W {
    pub out: Vec<u8>,
    explicit: bool,
    big: bool,
}

impl W {
    pub fn new(explicit: bool, big: bool) -> W {
        W {
            out: Vec::new(),
            explicit,
            big,
        }
    }

    fn u16(&mut self, v: u16) {
        self.out.extend(if self.big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        });
    }

    fn u32(&mut self, v: u32) {
        self.out.extend(if self.big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        });
    }

    fn header(&mut self, tag: u32, vr: &[u8; 2], len: u32) {
        self.u16((tag >> 16) as u16);
        self.u16(tag as u16);
        if self.explicit {
            self.out.extend(vr);
            if dict::Vr::from_bytes(*vr).unwrap().long_length() {
                self.u16(0);
                self.u32(len);
            } else {
                self.u16(len as u16);
            }
        } else {
            self.u32(len);
        }
    }

    pub fn raw(&mut self, tag: u32, vr: &[u8; 2], val: &[u8]) -> &mut W {
        let mut v = val.to_vec();
        if v.len() % 2 == 1 {
            v.push(if dict::Vr::from_bytes(*vr).unwrap().is_text() {
                b' '
            } else {
                0
            });
        }
        self.header(tag, vr, v.len() as u32);
        self.out.extend(v);
        self
    }

    pub fn s(&mut self, tag: u32, vr: &[u8; 2], s: &str) -> &mut W {
        self.raw(tag, vr, s.as_bytes())
    }

    pub fn us(&mut self, tag: u32, vals: &[u16]) -> &mut W {
        let mut b = Vec::new();
        for &v in vals {
            b.extend(if self.big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            });
        }
        self.raw(tag, b"US", &b)
    }

    pub fn ow(&mut self, tag: u32, vals: &[u16]) -> &mut W {
        let mut b = Vec::new();
        for &v in vals {
            b.extend(if self.big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            });
        }
        self.raw(tag, b"OW", &b)
    }

    /// A sequence of undefined length with undefined-length items.
    pub fn seq(&mut self, tag: u32, items: &[W]) -> &mut W {
        self.header(tag, b"SQ", 0xFFFF_FFFF);
        for it in items {
            self.u16(0xFFFE);
            self.u16(0xE000);
            self.u32(0xFFFF_FFFF);
            self.out.extend(&it.out);
            self.u16(0xFFFE);
            self.u16(0xE00D);
            self.u32(0);
        }
        self.u16(0xFFFE);
        self.u16(0xE0DD);
        self.u32(0);
        self
    }

    /// A sequence of defined length with defined-length items.
    pub fn seq_defined(&mut self, tag: u32, items: &[W]) -> &mut W {
        let len: usize = items.iter().map(|i| 8 + i.out.len()).sum();
        self.header(tag, b"SQ", len as u32);
        for it in items {
            self.u16(0xFFFE);
            self.u16(0xE000);
            self.u32(it.out.len() as u32);
            self.out.extend(&it.out);
        }
        self
    }

    pub fn encapsulated(&mut self, bot: &[u32], frags: &[Vec<u8>]) -> &mut W {
        self.header(PIXEL_DATA, b"OB", 0xFFFF_FFFF);
        self.u16(0xFFFE);
        self.u16(0xE000);
        self.u32(bot.len() as u32 * 4);
        for &o in bot {
            self.out.extend(o.to_le_bytes());
        }
        for f in frags {
            let mut f = f.clone();
            if f.len() % 2 == 1 {
                f.push(0);
            }
            self.u16(0xFFFE);
            self.u16(0xE000);
            self.u32(f.len() as u32);
            self.out.extend(f);
        }
        self.u16(0xFFFE);
        self.u16(0xE0DD);
        self.u32(0);
        self
    }

    pub fn image(
        &mut self,
        rows: u16,
        cols: u16,
        samples: u16,
        pm: &str,
        bits: (u16, u16, u16),
        signed: bool,
    ) -> &mut W {
        self.us(0x0028_0002, &[samples])
            .s(0x0028_0004, b"CS", pm)
            .us(0x0028_0010, &[rows])
            .us(0x0028_0011, &[cols])
            .us(0x0028_0100, &[bits.0])
            .us(0x0028_0101, &[bits.1])
            .us(0x0028_0102, &[bits.2])
            .us(0x0028_0103, &[signed as u16])
    }
}

/// Preamble, `DICM` and file meta information for `ts`, then `body`.
pub(crate) fn file(ts: &str, body: &[u8]) -> Vec<u8> {
    let mut meta = W::new(true, false);
    meta.raw(0x0002_0001, b"OB", &[0, 1])
        .s(0x0002_0002, b"UI", "1.2.840.10008.5.1.4.1.1.7")
        .s(0x0002_0010, b"UI", ts);
    let mut out = vec![0u8; 128];
    out.extend(b"DICM");
    let mut g = W::new(true, false);
    g.raw(0x0002_0000, b"UL", &(meta.out.len() as u32).to_le_bytes());
    out.extend(g.out);
    out.extend(meta.out);
    out.extend(body);
    out
}

const EXPLICIT: &str = "1.2.840.10008.1.2.1";

fn ct_body(w: &mut W) {
    w.s(0x0008_0005, b"CS", "ISO_IR 100")
        .s(0x0008_0060, b"CS", "CT")
        .s(0x0008_1030, b"LO", "Thorax")
        .s(0x0010_0010, b"PN", "Doe^Jane")
        .s(0x0018_0015, b"CS", "CHEST")
        .s(0x0018_0050, b"DS", "1.25")
        .image(2, 3, 1, "MONOCHROME2", (16, 12, 11), true)
        .s(0x0028_0030, b"DS", "0.5\\0.6")
        .s(0x0028_1050, b"DS", "40\\400")
        .s(0x0028_1051, b"DS", "400\\2000")
        .s(0x0028_1052, b"DS", "-1024")
        .s(0x0028_1053, b"DS", "1");
}

fn ct_values() -> Vec<i16> {
    vec![-1024, 0, 1024, 2047, -2048, 1064]
}

fn ct_pixels(big: bool) -> Vec<u8> {
    ct_values()
        .iter()
        .flat_map(|&v| {
            // 12 bits stored; garbage in the unused high bits must be masked off.
            let raw = (v as u16 & 0x0FFF) | 0xA000;
            if big {
                raw.to_be_bytes()
            } else {
                raw.to_le_bytes()
            }
        })
        .collect()
}

fn check_ct(d: &Dicom, file: &[u8]) {
    let mf = d.modality_frame(file, 0, None).unwrap();
    let hu: Vec<f64> = (0..6).map(|i| mf.value_at(i % 3, i / 3).unwrap()).collect();
    let want: Vec<f64> = ct_values().iter().map(|&v| v as f64 - 1024.0).collect();
    assert_eq!(hu, want);
    // The GPU's copy: the same values in f32, exact for integer HU.
    let f: Vec<f64> = mf.values_f32().unwrap().iter().map(|&v| v as f64).collect();
    assert_eq!(f, want);
    assert_eq!(mf.unit(), Some("HU"));
    let (voi, src) = mf.default_voi();
    assert_eq!(src, VoiSource::Window);
    let img = mf.render(&voi).unwrap();
    let Rendered::Gray16(g) = &img.pixels else {
        panic!()
    };
    // Window 40 / 400 (LINEAR): -1024 HU black, 1023 HU white, 0 HU at
    // (0 - 39.5) / 399 + 0.5, 40 HU at 0.5 / 399 + 0.5.
    assert_eq!(&g[..2], &[0, 0]);
    assert_eq!(g[2], 26280);
    assert_eq!(g[3], 65535);
    assert_eq!(g[5], 32850);
    let s = d.summary(file);
    assert_eq!(s.modality.as_deref(), Some("CT"));
    assert_eq!(s.pixel_spacing, Some((0.5, 0.6)));
    assert_eq!(s.slice_thickness, Some(1.25));
    assert_eq!(s.body_part.as_deref(), Some("CHEST"));
    // Rows 0.5 mm apart, columns 0.6 mm: shown 0.5 / 0.6 times as tall as wide.
    assert!((s.pixel_aspect() - 0.5 / 0.6).abs() < 1e-12);
    assert!(
        format!("{s:?}").find("Doe").is_none(),
        "no patient data in the summary"
    );
}

#[test]
fn explicit_little_endian_ct() {
    let mut w = W::new(true, false);
    ct_body(&mut w);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(false));
    let f = file(EXPLICIT, &w.out);
    assert!(sniff(&f));
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(d.transfer_syntax.encoding, Encoding::ExplicitLittle);
    check_ct(&d, &f);
}

#[test]
fn implicit_and_big_endian_ct() {
    let mut w = W::new(false, false);
    ct_body(&mut w);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(false));
    let f = file("1.2.840.10008.1.2", &w.out);
    check_ct(&Dicom::parse(&f).unwrap(), &f);

    let mut w = W::new(true, true);
    ct_body(&mut w);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(true));
    let f = file("1.2.840.10008.1.2.2", &w.out);
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(d.transfer_syntax.encoding, Encoding::ExplicitBig);
    check_ct(&d, &f);
}

#[test]
fn no_preamble_and_no_meta() {
    let mut w = W::new(false, false);
    ct_body(&mut w);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(false));
    assert!(sniff(&w.out));
    let d = Dicom::parse(&w.out).unwrap();
    assert!(d.meta.elements.is_empty());
    check_ct(&d, &w.out);
    // Explicit VR without meta is detected from the bytes.
    let mut w = W::new(true, false);
    ct_body(&mut w);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(false));
    let d = Dicom::parse(&w.out).unwrap();
    assert_eq!(d.transfer_syntax.encoding, Encoding::ExplicitLittle);
    check_ct(&d, &w.out);
    assert!(!sniff(b"\x89PNG\r\n\x1a\n0000000000000000000000"));
}

/// A raw deflate stream of stored blocks.
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, c) in chunks.iter().enumerate() {
        out.push((i + 1 == chunks.len()) as u8);
        out.extend((c.len() as u16).to_le_bytes());
        out.extend((!(c.len() as u16)).to_le_bytes());
        out.extend(*c);
    }
    out
}

fn deflated_fixture() -> Vec<u8> {
    let mut w = W::new(true, false);
    ct_body(&mut w);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(false));
    file("1.2.840.10008.1.2.1.99", &deflate_stored(&w.out))
}

#[test]
fn deflated_data_set_is_held_against_the_budget() {
    let f = deflated_fixture();
    let r = specular_core::budget::with_limit(512, || Dicom::parse(&f).map(|_| ()));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
    let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(1 << 30));
    let d = specular_core::budget::with_budget(b.clone(), || Dicom::parse(&f).unwrap());
    let held = b.reserved();
    assert!(held > 0, "the inflated data set holds no reservation");
    let d2 = d.clone();
    drop(d);
    assert_eq!(b.reserved(), held);
    drop(d2);
    assert_eq!(b.reserved(), 0);
}

#[test]
fn deflated_explicit_little_endian() {
    let f = deflated_fixture();
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(d.transfer_syntax.encoding, Encoding::DeflatedLittle);
    check_ct(&d, &f);
}

#[test]
fn sequences_and_icon() {
    // Nested sequences of both length kinds, an unknown private undefined-length element,
    // and an 8-bit icon.
    let mut inner = W::new(true, false);
    inner.s(0x0008_0100, b"SH", "T-D3000");
    let mut item = W::new(true, false);
    item.seq_defined(0x0008_2218, &[inner.clone(), inner])
        .s(0x0008_0104, b"LO", "x");
    let mut icon = W::new(true, false);
    icon.image(2, 2, 1, "MONOCHROME2", (8, 8, 7), false)
        .raw(PIXEL_DATA, b"OB", &[0, 85, 170, 255]);
    let mut w = W::new(true, false);
    w.s(0x0008_0060, b"CS", "OT")
        .seq(0x0008_1032, &[item.clone(), item])
        .image(1, 2, 1, "MONOCHROME2", (8, 8, 7), false)
        .seq(0x0088_0200, &[icon])
        .raw(PIXEL_DATA, b"OB", &[10, 20]);
    let f = file(EXPLICIT, &w.out);
    let d = Dicom::parse(&f).unwrap();
    let seq = d.dataset.get(0x0008_1032).unwrap();
    assert_eq!(seq.items.len(), 2);
    assert_eq!(seq.items[0].get(0x0008_2218).unwrap().items.len(), 2);
    let icon = d.icon(&f).unwrap().unwrap();
    assert_eq!((icon.width, icon.height), (2, 2));
    assert_eq!(icon.pixels, Rendered::Gray16(vec![0, 21845, 43690, 65535]));
    let mf = d.modality_frame(&f, 0, None).unwrap();
    assert_eq!(mf.default_voi().1, VoiSource::MinMax);
    assert_eq!(
        mf.render_default().unwrap().pixels,
        Rendered::Gray16(vec![0, 65535])
    );
}

fn rle_frame(vals: &[u8]) -> Vec<u8> {
    rle::encode(&[vals.to_vec()])
}

#[test]
fn rle_multiframe_with_and_without_tables() {
    let frames: Vec<Vec<u8>> = (0..3u8).map(|k| vec![k, k + 1, k + 2, k + 3]).collect();
    let encoded: Vec<Vec<u8>> = frames.iter().map(|f| rle_frame(f)).collect();
    let mut offs = Vec::new();
    let mut o = 0u32;
    for e in &encoded {
        offs.push(o);
        o += 8 + e.len().next_multiple_of(2) as u32;
    }
    // Each frame split in two fragments, mapped through the Basic Offset Table.
    let mut split = Vec::new();
    let mut boffs = Vec::new();
    let mut o = 0u32;
    for e in &encoded {
        boffs.push(o);
        let (a, b) = e.split_at(64);
        for part in [a, b] {
            split.push(part.to_vec());
            o += 8 + part.len().next_multiple_of(2) as u32;
        }
    }
    for (bot, frags) in [
        (vec![], encoded.clone()),
        (offs, encoded.clone()),
        (boffs, split),
    ] {
        let mut w = W::new(true, false);
        w.image(2, 2, 1, "MONOCHROME2", (8, 8, 7), false)
            .s(0x0028_0008, b"IS", "3")
            .encapsulated(&bot, &frags);
        let f = file("1.2.840.10008.1.2.5", &w.out);
        let d = Dicom::parse(&f).unwrap();
        assert_eq!(d.frame_count(), 3);
        for (k, want) in frames.iter().enumerate() {
            let fr = d.decode_frame(&f, k as u32, None).unwrap();
            let want: Vec<u16> = want.iter().map(|&v| v as u16).collect();
            assert_eq!(
                fr.pixels,
                Pixels::Grey(Grey::U16(want)),
                "frame {k} bot {bot:?}"
            );
        }
        assert!(d.decode_frame(&f, 3, None).is_err());
    }
}

#[test]
fn deflated_image_frames_and_one_bit_rle() {
    // Deflated Image Frame Compression: each frame raw-deflated in its own fragment.
    let mut w = W::new(true, false);
    w.image(2, 3, 1, "MONOCHROME2", (16, 12, 11), false)
        .s(0x0028_0008, b"IS", "2");
    let f0: Vec<u8> = [1u16, 2, 3, 4, 5, 4095]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let f1: Vec<u8> = [9u16, 8, 7, 6, 5, 4]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    w.encapsulated(&[], &[deflate_stored(&f0), deflate_stored(&f1)]);
    let f = file("1.2.840.10008.1.2.8.1", &w.out);
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(d.transfer_syntax.name, "Deflated Image Frame Compression");
    let fr = d.decode_frame(&f, 1, None).unwrap();
    let Pixels::Grey(g) = &fr.pixels else {
        panic!()
    };
    assert_eq!(
        (0..6).map(|i| g.get(i).unwrap()).collect::<Vec<_>>(),
        [9, 8, 7, 6, 5, 4]
    );

    // 1-bit RLE, both as packed bits and as one byte per pixel.
    let bits = [1u8, 0, 1, 1, 0, 0, 0, 1, 1];
    let packed = vec![0b1000_1101u8, 1];
    for seg in [packed, bits.to_vec()] {
        let mut w = W::new(true, false);
        w.image(3, 3, 1, "MONOCHROME2", (1, 1, 0), false)
            .encapsulated(&[], &[rle_frame(&seg)]);
        let f = file("1.2.840.10008.1.2.5", &w.out);
        let d = Dicom::parse(&f).unwrap();
        let fr = d.decode_frame(&f, 0, None).unwrap();
        let Pixels::Grey(g) = &fr.pixels else {
            panic!()
        };
        let got: Vec<i64> = (0..9).map(|i| g.get(i).unwrap()).collect();
        assert_eq!(got, bits.map(i64::from), "segment {seg:?}");
    }
}

#[test]
fn colour_photometrics() {
    // RGB, colour-by-plane.
    let mut w = W::new(true, false);
    w.image(1, 2, 3, "RGB", (8, 8, 7), false)
        .us(0x0028_0006, &[1])
        .raw(PIXEL_DATA, b"OB", &[1, 2, 3, 4, 5, 6]);
    let f = file(EXPLICIT, &w.out);
    let fr = Dicom::parse(&f).unwrap().decode_frame(&f, 0, None).unwrap();
    assert_eq!(fr.pixels, Pixels::Rgb8(vec![1, 3, 5, 2, 4, 6]));

    // YBR_FULL grey and YBR_FULL_422 (Y0 Y1 Cb Cr).
    let mut w = W::new(true, false);
    w.image(1, 2, 3, "YBR_FULL", (8, 8, 7), false).raw(
        PIXEL_DATA,
        b"OB",
        &[100, 128, 128, 76, 85, 255],
    );
    let f = file(EXPLICIT, &w.out);
    let fr = Dicom::parse(&f).unwrap().decode_frame(&f, 0, None).unwrap();
    let Pixels::Rgb8(p) = fr.pixels else { panic!() };
    assert_eq!(&p[..3], &[100, 100, 100]);
    assert!(p[3] > 250 && p[4] < 5 && p[5] < 5, "{p:?}");
    let mut w = W::new(true, false);
    w.image(1, 2, 3, "YBR_FULL_422", (8, 8, 7), false)
        .raw(PIXEL_DATA, b"OB", &[50, 60, 128, 128]);
    let f = file(EXPLICIT, &w.out);
    let fr = Dicom::parse(&f).unwrap().decode_frame(&f, 0, None).unwrap();
    assert_eq!(fr.pixels, Pixels::Rgb8(vec![50, 50, 50, 60, 60, 60]));

    // Palette colour, one channel segmented.
    let mut w = W::new(true, false);
    w.image(1, 3, 1, "PALETTE COLOR", (8, 8, 7), false)
        .us(0x0028_1101, &[3, 5, 16])
        .us(0x0028_1102, &[3, 5, 16])
        .us(0x0028_1103, &[3, 5, 16])
        .ow(0x0028_1201, &[0, 1000, 65535])
        .ow(0x0028_1202, &[7, 8, 9])
        .ow(0x0028_1223, &[0, 1, 100, 1, 2, 300])
        .raw(PIXEL_DATA, b"OB", &[4, 6, 9]);
    let f = file(EXPLICIT, &w.out);
    let fr = Dicom::parse(&f).unwrap().decode_frame(&f, 0, None).unwrap();
    assert_eq!(
        fr.pixels,
        Pixels::Rgb16(vec![0, 7, 100, 1000, 8, 200, 65535, 9, 300])
    );
}

#[test]
fn monochrome1_inverts_and_lut_modality() {
    let mut lut_item = W::new(true, false);
    lut_item
        .us(0x0028_3002, &[4, 0, 16])
        .s(0x0028_3004, b"LO", "x")
        .ow(0x0028_3006, &[100, 200, 300, 400]);
    let mut w = W::new(true, false);
    w.image(1, 4, 1, "MONOCHROME1", (8, 8, 7), false)
        .seq(0x0028_3000, &[lut_item])
        .raw(PIXEL_DATA, b"OB", &[0, 1, 2, 9]);
    let f = file(EXPLICIT, &w.out);
    let d = Dicom::parse(&f).unwrap();
    let mf = d.modality_frame(&f, 0, None).unwrap();
    assert_eq!(mf.value_at(3, 0), Some(400.0));
    assert_eq!(mf.values_f32(), Some(vec![100.0, 200.0, 300.0, 400.0]));
    assert_eq!(mf.value_range(), Some((100.0, 400.0)));
    assert!(mf.inverted());
    assert_eq!(
        mf.render_default().unwrap().pixels,
        Rendered::Gray16(vec![65535, 43690, 21845, 0])
    );
}

#[test]
fn one_bit_and_32_bit() {
    let mut w = W::new(true, false);
    w.image(2, 8, 1, "MONOCHROME2", (1, 1, 0), false)
        .s(0x0028_0008, b"IS", "2")
        .raw(PIXEL_DATA, b"OB", &[0b0000_0101, 0xFF, 0x80, 0]);
    let f = file(EXPLICIT, &w.out);
    let d = Dicom::parse(&f).unwrap();
    let fr = d.decode_frame(&f, 0, None).unwrap();
    let Pixels::Grey(Grey::U16(g)) = fr.pixels else {
        panic!()
    };
    assert_eq!(&g[..8], &[1, 0, 1, 0, 0, 0, 0, 0]);
    let fr = d.decode_frame(&f, 1, None).unwrap();
    let Pixels::Grey(Grey::U16(g)) = fr.pixels else {
        panic!()
    };
    assert_eq!(&g[..8], &[0, 0, 0, 0, 0, 0, 0, 1]);

    let mut w = W::new(true, false);
    let vals: Vec<u8> = [-5i32, 100_000]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    w.image(1, 2, 1, "MONOCHROME2", (32, 32, 31), true)
        .raw(PIXEL_DATA, b"OB", &vals);
    let f = file(EXPLICIT, &w.out);
    let fr = Dicom::parse(&f).unwrap().decode_frame(&f, 0, None).unwrap();
    assert_eq!(fr.pixels, Pixels::Grey(Grey::I32(vec![-5, 100_000])));
}

/// The worked example of T.87 Annex H.3 (4×4, 8-bit, lossless), with the final SOF55 code.
const JLS_H3: [u8; 57] = [
    0xFF, 0xD8, 0xFF, 0xF7, 0x00, 0x0B, 0x08, 0x00, 0x04, 0x00, 0x04, 0x01, 0x01, 0x11, 0x00, 0xFF,
    0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x6C, 0x80, 0x20, 0x8E,
    0x01, 0xC0, 0x00, 0x00, 0x57, 0x40, 0x00, 0x00, 0x6E, 0xE6, 0x00, 0x00, 0x01, 0xBC, 0x18, 0x00,
    0x00, 0x05, 0xD8, 0x00, 0x00, 0x91, 0x60, 0xFF, 0xD9,
];

#[test]
fn jpeg_ls_frames() {
    let mut w = W::new(true, false);
    w.image(4, 4, 1, "MONOCHROME2", (8, 8, 7), false)
        .s(0x0028_0008, b"IS", "2")
        .encapsulated(&[], &[JLS_H3.to_vec(), JLS_H3[..40].to_vec()]);
    let f = file("1.2.840.10008.1.2.4.80", &w.out);
    let d = Dicom::parse(&f).unwrap();
    d.check_supported().unwrap();
    let fr = d.decode_frame(&f, 0, None).unwrap();
    assert!(!fr.truncated);
    let Pixels::Grey(Grey::U16(g)) = fr.pixels else {
        panic!("{:?}", fr.pixels)
    };
    assert_eq!(
        g,
        [
            0, 0, 90, 74, 68, 50, 43, 205, 64, 145, 145, 145, 100, 145, 145, 145
        ]
    );
    // A cut codestream gives the decoded lines.
    let fr = d.decode_frame(&f, 1, None).unwrap();
    assert!(fr.truncated);
    // A codestream of another size is refused.
    let mut w = W::new(true, false);
    w.image(1, 1, 1, "MONOCHROME2", (8, 8, 7), false)
        .encapsulated(&[], &[JLS_H3.to_vec()]);
    let f = file("1.2.840.10008.1.2.4.81", &w.out);
    let d = Dicom::parse(&f).unwrap();
    assert!(d.decode_frame(&f, 0, None).is_err());
}

/// The retired JPEG syntaxes: specular-jpeg's fixtures (arithmetic, progressive and
/// hierarchical, 8 and 12 bits; oracle-checked in its `tests/wide.rs`) wrapped as frames.
#[test]
fn retired_jpeg_frames() {
    macro_rules! fx {
        ($f:literal) => {
            include_bytes!(concat!("../../specular-jpeg/tests/fixtures/", $f)).as_slice()
        };
    }
    // (syntax, stream, samples, photometric)
    let cases: &[(&str, &[u8], u16, &str)] = &[
        (
            "1.2.840.10008.1.2.4.52",
            fx!("dac12_420_rst.jpg"),
            3,
            "YBR_FULL_422",
        ),
        (
            "1.2.840.10008.1.2.4.56",
            fx!("dac12_444_prog.jpg"),
            3,
            "YBR_FULL",
        ),
        (
            "1.2.840.10008.1.2.4.54",
            fx!("arith_420_prog.jpg"),
            3,
            "YBR_FULL_422",
        ),
        (
            "1.2.840.10008.1.2.4.59",
            fx!("hier/grey12_y2.jpg"),
            1,
            "MONOCHROME2",
        ),
        (
            "1.2.840.10008.1.2.4.63",
            fx!("hier/grey_y2_prog.jpg"),
            1,
            "MONOCHROME2",
        ),
        (
            "1.2.840.10008.1.2.4.64",
            fx!("hier/grey_y3_arith.jpg"),
            1,
            "MONOCHROME2",
        ),
        (
            "1.2.840.10008.1.2.4.65",
            fx!("hier/grey_y0_lossless.jpg"),
            1,
            "MONOCHROME2",
        ),
        (
            "1.2.840.10008.1.2.4.60",
            fx!("hier/rgb_y2_420.jpg"),
            3,
            "YBR_FULL_422",
        ),
    ];
    for &(ts, jpg, samples, pm) in cases {
        let head = specular_jpeg::read_info(jpg).unwrap();
        let bits = head.precision as u16;
        let mut w = W::new(true, false);
        w.image(
            head.height as u16,
            head.width as u16,
            samples,
            pm,
            (if bits > 8 { 16 } else { 8 }, bits, bits - 1),
            false,
        )
        .encapsulated(&[], &[jpg.to_vec()]);
        let f = file(ts, &w.out);
        let d = Dicom::parse(&f).unwrap();
        d.check_supported().unwrap_or_else(|e| panic!("{ts}: {e}"));
        let fr = d
            .decode_frame(&f, 0, None)
            .unwrap_or_else(|e| panic!("{ts}: {e}"));
        assert!(!fr.truncated, "{ts}");
        assert_eq!((fr.width, fr.height), (head.width, head.height), "{ts}");
        if samples == 1 && bits > 8 {
            // Stored values, not a display reduction.
            let want = specular_jpeg::decode_wide(jpg, None).unwrap();
            let Pixels::Grey(Grey::U16(g)) = &fr.pixels else {
                panic!("{ts}: {:?}", fr.pixels)
            };
            assert_eq!(g, &want.planes[0].data, "{ts}");
            assert!(g.iter().any(|&v| v > 255), "{ts}");
        }
    }
    // Arithmetic-coded lossless frames in a hierarchical image (.66) are refused by name
    // when decoded (no encoder or file exists to check a decoder against).
    let e = specular_jpeg::decode_wide(&[0xFF, 0xD8, 0xFF, 0xD9], None).unwrap_err();
    assert!(!e.to_string().is_empty());
}

#[test]
fn refusals_are_named() {
    let mut w = W::new(true, false);
    w.image(1, 1, 1, "MONOCHROME2", (8, 8, 7), false)
        .encapsulated(&[], &[vec![0xFF, 0xD8, 0xFF, 0xF7]]);
    let f = file("1.2.840.10008.1.2.4.102", &w.out);
    let e = Dicom::parse(&f).unwrap().check_supported().unwrap_err();
    assert!(e.to_string().contains("H.264"), "{e}");
    assert_eq!(
        Dicom::parse(b"not dicom at all, really").unwrap_err(),
        Error::NotDicom
    );
}

#[test]
fn truncation_and_mutation_never_panic() {
    let mut w = W::new(true, false);
    ct_body(&mut w);
    let mut item = W::new(true, false);
    item.s(0x0008_0100, b"SH", "abc");
    w.seq(0x0008_1032, &[item.clone(), item]);
    w.raw(PIXEL_DATA, b"OW", &ct_pixels(false));
    let good = file(EXPLICIT, &w.out);
    let mut rle = W::new(true, false);
    rle.image(2, 2, 1, "MONOCHROME2", (8, 8, 7), false)
        .s(0x0028_0008, b"IS", "2")
        .encapsulated(&[], &[rle_frame(&[1, 2, 3, 4]), rle_frame(&[5, 6, 7, 8])]);
    let good_rle = file("1.2.840.10008.1.2.5", &rle.out);
    // `SPECULAR_DICOM_FUZZ_SEEDS=<dir>` refreshes the fuzz seeds from these fixtures.
    if let Some(dir) = std::env::var_os("SPECULAR_DICOM_FUZZ_SEEDS") {
        let dir = std::path::Path::new(&dir);
        std::fs::write(dir.join("ct_explicit.dcm"), &good).unwrap();
        std::fs::write(dir.join("rle_2frame.dcm"), &good_rle).unwrap();
        std::fs::write(dir.join("deflated.dcm"), deflated_fixture()).unwrap();
    }
    let exercise = |data: &[u8]| {
        if let Ok(d) = Dicom::parse(data) {
            let _ = d.summary(data);
            let _ = d.icon(data);
            for k in 0..d.frame_count().min(3) {
                if let Ok(mf) = d.modality_frame(data, k, None) {
                    let _ = mf.render_default();
                }
            }
        }
    };
    for base in [&good, &good_rle] {
        for n in 0..base.len() {
            exercise(&base[..n]);
        }
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..3000 {
            let mut m = base.clone();
            for _ in 0..4 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let i = (x as usize >> 8) % m.len();
                m[i] = (x >> 40) as u8;
            }
            exercise(&m);
        }
    }
}

/// An 8×6 unsigned 12-bit grey image of `frames` frames (value = 100 + pixel index + 1000 ×
/// frame), with `extra` elements.
fn graphics_file(frames: u16, extra: impl Fn(&mut W), embedded: Option<u16>) -> Vec<u8> {
    let mut w = W::new(true, false);
    w.image(6, 8, 1, "MONOCHROME2", (16, 12, 11), false);
    if frames > 1 {
        w.s(0x0028_0008, b"IS", &frames.to_string());
    }
    extra(&mut w);
    let px: Vec<u16> = (0..frames)
        .flat_map(|f| (0..48u16).map(move |i| 100 + i + 1000 * f))
        .map(|v| match embedded {
            // The overlay bit on every third pixel.
            Some(bit) => v | (u16::from(v % 3 == 0) << bit),
            None => v,
        })
        .collect();
    w.ow(PIXEL_DATA, &px);
    file(EXPLICIT, &w.out)
}

#[test]
fn shutters_and_overlays_burn_in() {
    use crate::graphics::{OVERLAY, SHUTTER};
    let f = graphics_file(
        1,
        |w| {
            w.s(0x0018_1600, b"CS", "RECTANGULAR\\CIRCULAR")
                .s(0x0018_1602, b"IS", "2")
                .s(0x0018_1604, b"IS", "7")
                .s(0x0018_1606, b"IS", "1")
                .s(0x0018_1608, b"IS", "5")
                .s(0x0018_1610, b"IS", "3\\4")
                .s(0x0018_1612, b"IS", "3")
                .us(0x0018_1622, &[1000])
                .us(0x6000_0010, &[2])
                .us(0x6000_0011, &[3])
                .s(0x6000_0040, b"CS", "G")
                .raw(0x6000_0050, b"SS", &[2, 0, 3, 0])
                .us(0x6000_0100, &[1])
                .us(0x6000_0102, &[0])
                .raw(0x6000_3000, b"OB", &[0b0011_0101, 0]);
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    let m = d.modality_frame(&f, 0, None).unwrap();
    let g = m.graphics().unwrap();
    assert!(g.shutter && g.overlays == 1);
    // Overlay bits 0, 2, 4, 5 of a 3×2 plane at row 2, column 3 (1-based).
    let set: Vec<(u32, u32)> = (0..6)
        .flat_map(|y| (0..8).map(move |x| (x, y)))
        .filter(|&(x, y)| g.at(x, y) & OVERLAY != 0)
        .collect();
    assert_eq!(set, vec![(2, 1), (4, 1), (3, 2), (4, 2)]);
    // Outside the rectangle (column 1, column 8), outside the circle (row 1, column 7).
    assert_eq!(g.at(0, 0) & SHUTTER, SHUTTER);
    assert_eq!(g.at(7, 4) & SHUTTER, SHUTTER);
    assert_eq!(g.at(6, 0) & SHUTTER, SHUTTER);
    assert_eq!(g.at(3, 2) & SHUTTER, 0);
    let voi = m.default_voi().0;
    let Rendered::Gray16(p) = m.render(&voi).unwrap().pixels else {
        panic!()
    };
    assert_eq!(
        (p[0], p[2 * 8 + 3], p[3 * 8 + 3]),
        (1000, 65535, p[3 * 8 + 3])
    );
    assert!(p[3 * 8 + 3] != 1000 && p[3 * 8 + 3] != 65535);
    let Rendered::Gray16(q) = m.render_with(&voi, false).unwrap().pixels else {
        panic!()
    };
    assert_eq!(q[0], 1000);
    assert_ne!(q[2 * 8 + 3], 65535);
    // Without either: no graphics.
    let plain = graphics_file(1, |_| {}, None);
    let d = Dicom::parse(&plain).unwrap();
    assert!(
        d.modality_frame(&plain, 0, None)
            .unwrap()
            .graphics()
            .is_none()
    );
}

#[test]
fn polygonal_shutter() {
    use crate::graphics::SHUTTER;
    let f = graphics_file(
        1,
        |w| {
            w.s(0x0018_1600, b"CS", "POLYGONAL")
                .s(0x0018_1620, b"IS", "1\\1\\1\\8\\6\\1");
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    let g = d
        .modality_frame(&f, 0, None)
        .unwrap()
        .graphics()
        .cloned()
        .unwrap();
    // The triangle (row, col) (1, 1), (1, 8), (6, 1): the top-left half shows.
    assert_eq!(g.at(1, 1) & SHUTTER, 0);
    assert_eq!(g.at(7, 5) & SHUTTER, SHUTTER);
}

#[test]
fn multi_frame_and_embedded_overlays() {
    use crate::graphics::OVERLAY;
    // A 2-frame overlay plane over image frames 2 and 3 (1-based).
    let f = graphics_file(
        3,
        |w| {
            w.us(0x6002_0010, &[2])
                .us(0x6002_0011, &[2])
                .s(0x6002_0015, b"IS", "2")
                .s(0x6002_0040, b"CS", "G")
                .raw(0x6002_0050, b"SS", &[1, 0, 1, 0])
                .us(0x6002_0051, &[2])
                .us(0x6002_0100, &[1])
                .us(0x6002_0102, &[0])
                .raw(0x6002_3000, b"OB", &[0b1000_0001, 0]);
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    assert!(d.modality_frame(&f, 0, None).unwrap().graphics().is_none());
    let g1 = d
        .modality_frame(&f, 1, None)
        .unwrap()
        .graphics()
        .cloned()
        .unwrap();
    assert_eq!((g1.at(0, 0), g1.at(1, 1)), (OVERLAY, 0));
    let g2 = d
        .modality_frame(&f, 2, None)
        .unwrap()
        .graphics()
        .cloned()
        .unwrap();
    assert_eq!((g2.at(0, 0), g2.at(1, 1)), (0, OVERLAY));

    // Retired: bit 15 of the pixel cells, above the 12 stored bits.
    let f = graphics_file(
        2,
        |w| {
            w.us(0x6000_0010, &[6])
                .us(0x6000_0011, &[8])
                .s(0x6000_0040, b"CS", "G")
                .raw(0x6000_0050, b"SS", &[1, 0, 1, 0])
                .us(0x6000_0100, &[16])
                .us(0x6000_0102, &[15]);
        },
        Some(15),
    );
    let d = Dicom::parse(&f).unwrap();
    for frame in 0..2u32 {
        let m = d.modality_frame(&f, frame, None).unwrap();
        let g = m.graphics().unwrap();
        for i in 0..48u32 {
            let v = 100 + i + 1000 * frame;
            assert_eq!(g.at(i % 8, i / 8) & OVERLAY != 0, v % 3 == 0, "pixel {i}");
            // The stored value is masked to 12 bits.
            assert_eq!(m.stored_at(i % 8, i / 8), Some(i64::from(v)));
        }
    }
}

#[test]
fn spacing_kinds_and_frame_times() {
    use crate::calib::SpacingKind;
    let spacing = |extra: &dyn Fn(&mut W)| {
        let f = graphics_file(1, extra, None);
        let d = Dicom::parse(&f).unwrap();
        d.modality_frame(&f, 0, None)
            .unwrap()
            .calibration()
            .spacing
            .map(|s| (s.x, s.y, s.kind))
    };
    assert_eq!(spacing(&|_| {}), None);
    assert_eq!(
        spacing(&|w| {
            w.s(0x0028_0030, b"DS", "0.5\\0.6");
        }),
        Some((0.6, 0.5, SpacingKind::Patient))
    );
    assert_eq!(
        spacing(&|w| {
            w.s(0x0018_1114, b"DS", "1.25")
                .s(0x0018_1164, b"DS", "0.2\\0.25");
        }),
        Some((0.2, 0.16, SpacingKind::Magnification))
    );
    assert_eq!(
        spacing(&|w| {
            w.s(0x0018_1164, b"DS", "0.2\\0.2")
                .s(0x0028_0030, b"DS", "0.2\\0.2")
                .s(0x0028_0A02, b"CS", "GEOMETRY");
        }),
        Some((0.2, 0.2, SpacingKind::Calibrated))
    );
    assert_eq!(
        spacing(&|w| {
            w.s(0x0018_1164, b"DS", "0.2\\0.2")
                .s(0x0028_0030, b"DS", "0.18\\0.18");
        }),
        Some((0.18, 0.18, SpacingKind::Magnification))
    );
    // Enhanced: the shared Pixel Measures.
    assert_eq!(
        spacing(&|w| {
            let mut pm = W::new(true, false);
            pm.s(0x0028_0030, b"DS", "0.7\\0.8");
            let mut shared = W::new(true, false);
            shared.seq(0x0028_9110, &[pm]);
            w.seq(0x5200_9229, &[shared]);
        }),
        Some((0.8, 0.7, SpacingKind::Patient))
    );

    let f = graphics_file(
        3,
        |w| {
            w.s(0x0018_1063, b"DS", "25")
                .s(0x0018_1065, b"DS", "0\\40\\60");
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    let ms: Vec<u128> = d
        .frame_durations(&f)
        .unwrap()
        .iter()
        .map(|t| t.as_millis())
        .collect();
    assert_eq!(ms, vec![40, 60, 50]);
    let f = graphics_file(
        2,
        |w| {
            w.s(0x0018_0040, b"IS", "20");
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(
        d.frame_durations(&f).unwrap(),
        vec![std::time::Duration::from_millis(50); 2]
    );
}

/// The file's windows come with their explanations (Window Center & Width Explanation).
#[test]
fn window_explanations() {
    let f = graphics_file(
        1,
        |w| {
            w.s(0x0028_1050, b"DS", "40\\300\\50")
                .s(0x0028_1051, b"DS", "80\\1500\\350")
                .s(0x0028_1055, b"LO", "BRAIN\\BONE");
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    let m = d.modality_frame(&f, 0, None).unwrap();
    assert_eq!(m.windows(), &[(40.0, 80.0), (300.0, 1500.0), (50.0, 350.0)]);
    assert_eq!(m.window_explanation(0), Some("BRAIN"));
    assert_eq!(m.window_explanation(1), Some("BONE"));
    assert_eq!(m.window_explanation(2), None);
}

#[test]
fn ultrasound_regions_in_the_summary() {
    use crate::calib::Uncalibrated;
    let f = graphics_file(
        1,
        |w| {
            let mut r = W::new(true, false);
            let ul = |v: u32| v.to_le_bytes();
            r.us(0x0018_6012, &[1])
                .raw(0x0018_6018, b"UL", &ul(0))
                .raw(0x0018_601A, b"UL", &ul(0))
                .raw(0x0018_601C, b"UL", &ul(2))
                .raw(0x0018_601E, b"UL", &ul(7))
                .us(0x0018_6024, &[3])
                .us(0x0018_6026, &[3])
                .raw(0x0018_602C, b"FD", &0.05f64.to_le_bytes())
                .raw(0x0018_602E, b"FD", &0.1f64.to_le_bytes());
            w.seq(0x0018_6011, &[r]).s(0x0028_0034, b"IS", "2\\1");
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    let s = d.summary(&f);
    assert_eq!(s.calibration.spacing, None);
    assert_eq!(s.calibration.regions.len(), 1);
    // The regions differ from each other: the Pixel Aspect Ratio decides the display.
    assert_eq!(s.pixel_aspect(), 2.0);
    let p = |x, y| specular_measure::P { x, y };
    let line = specular_measure::Shape::Line(p(0.5, 0.5), p(2.5, 6.5));
    let cal = s.calibration.measure(&line.points()).unwrap();
    assert_eq!(cal.source, specular_measure::Source::DicomUltrasoundRegion);
    // 2 columns of 0.5 mm, 6 rows of 1 mm.
    let len = line.length(&cal).unwrap().value;
    assert!((len - 37f64.sqrt()).abs() < 1e-9, "{len}");
    let out = specular_measure::Shape::Line(p(0.5, 0.5), p(4.5, 0.5));
    assert_eq!(
        s.calibration.measure(&out.points()),
        Err(Uncalibrated::OutsideRegion)
    );
}

#[test]
fn roi_statistics_in_modality_units() {
    let f = graphics_file(
        1,
        |w| {
            w.s(0x0028_0030, b"DS", "0.5\\0.5")
                .s(0x0028_1052, b"DS", "-100")
                .s(0x0028_1053, b"DS", "2");
        },
        None,
    );
    let d = Dicom::parse(&f).unwrap();
    let m = d.modality_frame(&f, 0, None).unwrap();
    let rect = specular_measure::Shape::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 1.0,
    };
    let s = m.roi_stats(&rect).unwrap();
    // Stored 100, 101 → 100, 102.
    assert_eq!(
        (s.count, s.min, s.max, s.mean, s.sd),
        (2, 100.0, 102.0, 101.0, 1.0)
    );
    let cal = m.calibration().measure(&rect.points()).unwrap();
    let area = rect.area(&cal).unwrap();
    assert_eq!(
        (area.value, area.source),
        (0.5, specular_measure::Source::DicomPixelSpacing)
    );
    // Nothing covered: no statistics.
    let off = specular_measure::Shape::Rect {
        x0: -5.0,
        y0: -5.0,
        x1: -1.0,
        y1: -1.0,
    };
    assert_eq!(m.roi_stats(&off), None);
}

/// A 2×2 8-bit grey image with `extra` elements.
fn tiny(extra: impl FnOnce(&mut W)) -> Vec<u8> {
    let mut w = W::new(true, false);
    w.s(0x0008_0060, b"CS", "MR");
    extra(&mut w);
    w.s(0x0028_0004, b"CS", "MONOCHROME2")
        .us(0x0028_0002, &[1])
        .us(0x0028_0010, &[2])
        .us(0x0028_0011, &[2])
        .us(0x0028_0100, &[8])
        .us(0x0028_0101, &[8])
        .raw(PIXEL_DATA, b"OB", &[0, 1, 2, 3]);
    file(EXPLICIT, &w.out)
}

/// Orientation from Image Orientation (Patient), per-frame Plane Orientation, or Patient
/// Orientation letters.
#[test]
fn patient_axes_from_the_data_set() {
    let axes = |f: &[u8], k: u32| {
        let d = Dicom::parse(f).unwrap();
        d.modality_frame(f, k, None)
            .unwrap()
            .patient_axes()
            .copied()
    };
    // Coronal: rows to the left, columns to the feet.
    let f = tiny(|w| {
        w.s(0x0020_0037, b"DS", "1\\0\\0\\0\\0\\-1");
    });
    let a = axes(&f, 0).unwrap();
    assert!(!a.from_letters);
    assert_eq!(
        [a.label(1.0, 0.0), a.label(0.0, 1.0)],
        [Some("L".into()), Some("I".into())]
    );
    // Sagittal per-frame (shared group) beats nothing at the top level.
    let f = tiny(|w| {
        let mut po = W::new(true, false);
        po.s(0x0020_0037, b"DS", "0\\1\\0\\0\\0\\-1");
        let mut shared = W::new(true, false);
        shared.seq(0x0020_9116, &[po]);
        w.seq(0x5200_9229, &[shared]);
    });
    let a = axes(&f, 0).unwrap();
    assert_eq!(a.label(-1.0, 0.0).unwrap(), "A");
    // Oblique: 30° about the feet-head axis.
    let f = tiny(|w| {
        w.s(0x0020_0037, b"DS", "0.8660254\\0.5\\0\\-0.5\\0.8660254\\0");
    });
    assert_eq!(axes(&f, 0).unwrap().label(1.0, 0.0).unwrap(), "LP");
    // Projection radiograph: Patient Orientation only.
    let f = tiny(|w| {
        w.s(0x0020_0020, b"CS", "P\\FR");
    });
    let a = axes(&f, 0).unwrap();
    assert!(a.from_letters);
    assert_eq!(a.label(1.0, 0.0).unwrap(), "P");
    assert_eq!(a.label(0.0, -1.0).unwrap(), "SL");
    // Neither, or junk.
    assert!(axes(&tiny(|_| {}), 0).is_none());
    let f = tiny(|w| {
        w.s(0x0020_0020, b"CS", "Q\\Z");
    });
    assert!(axes(&f, 0).is_none());
}

/// Float (OF) and Double Float (OD) Pixel Data in both byte orders: values decode exactly,
/// rescale applies in floating point, padding and NaN stay out of the range, and windows
/// work on the real values.
#[test]
fn float_pixel_data() {
    use crate::display::{Voi, VoiFunction};
    let vals = [-1.5f64, 0.25, 1000.75, f64::NAN, -7.0, 3.5];
    for (double, big) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut w = W::new(true, big);
        w.s(0x0008_0060, b"CS", "MR")
            .s(0x0028_0004, b"CS", "MONOCHROME2")
            .us(0x0028_0002, &[1])
            .us(0x0028_0010, &[2])
            .us(0x0028_0011, &[3])
            .us(0x0028_0100, &[if double { 64 } else { 32 }])
            .s(0x0028_1052, b"DS", "10")
            .s(0x0028_1053, b"DS", "0.5");
        let pad = -7.0f64;
        let mut px = Vec::new();
        if double {
            let b = if big {
                pad.to_be_bytes()
            } else {
                pad.to_le_bytes()
            };
            w.raw(0x0028_0123, b"FD", &b);
            for v in vals {
                px.extend(if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                });
            }
            w.raw(parse::DOUBLE_FLOAT_PIXEL_DATA, b"OD", &px);
        } else {
            let p = pad as f32;
            let b = if big {
                p.to_be_bytes()
            } else {
                p.to_le_bytes()
            };
            w.raw(0x0028_0122, b"FL", &b);
            for v in vals {
                let v = v as f32;
                px.extend(if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                });
            }
            w.raw(parse::FLOAT_PIXEL_DATA, b"OF", &px);
        }
        let ts = if big { "1.2.840.10008.1.2.2" } else { EXPLICIT };
        let f = file(ts, &w.out);
        let d = Dicom::parse(&f).unwrap();
        let info = d.pixel.as_ref().unwrap();
        assert!(info.float && info.signed);
        let m = d.modality_frame(&f, 0, None).unwrap();
        let Pixels::Grey(g) = &m.frame.pixels else {
            panic!("not grey")
        };
        assert!(g.is_float());
        assert_eq!(matches!(g, Grey::F64(_)), double);
        assert_eq!(m.value_at(0, 0), Some(-1.5 * 0.5 + 10.0));
        assert_eq!(m.value_at(2, 0), Some(1000.75 * 0.5 + 10.0));
        assert!(m.value_at(0, 1).unwrap().is_nan());
        assert_eq!(m.stored_at(1, 1), Some(-7));
        // Range of the finite, unpadded values: -1.5 .. 1000.75.
        assert_eq!(m.value_range(), Some((9.25, 510.375)));
        // A window on the real values: 9.25 at the bottom, 510.375 at the top.
        let voi = Voi::Window {
            center: (9.25 + 510.375) / 2.0,
            width: 510.375 - 9.25,
            function: VoiFunction::LinearExact,
        };
        let img = m.render(&voi).unwrap();
        let Rendered::Gray16(p) = &img.pixels else {
            panic!("not grey")
        };
        assert_eq!((p[0], p[2], p[3]), (0, 65535, 0));
        let want = ((0.25 * 0.5 + 10.0 - 9.25) / 501.125 * 65535.0 + 0.5) as u16;
        assert_eq!(p[1], want);
        let f32s = m.values_f32().unwrap();
        assert_eq!(f32s[5], 11.75);
        assert_eq!(m.frame.bytes(), if double { 48 } else { 24 });
    }
    // Colour float pixel data is refused cleanly.
    let mut w = W::new(true, false);
    w.s(0x0028_0004, b"CS", "RGB")
        .us(0x0028_0002, &[3])
        .us(0x0028_0010, &[1])
        .us(0x0028_0011, &[1])
        .us(0x0028_0100, &[32])
        .raw(parse::FLOAT_PIXEL_DATA, b"OF", &[0; 12]);
    assert!(matches!(
        Dicom::parse(&file(EXPLICIT, &w.out)),
        Err(Error::Unsupported(_))
    ));
}

fn fl(vals: &[f32]) -> Vec<u8> {
    vals.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn sl(vals: &[i32]) -> Vec<u8> {
    vals.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A presentation state referencing frame 2 of image `1.2.3.4`, with `extra` elements.
fn gsps_file(extra: impl FnOnce(&mut W)) -> Vec<u8> {
    let mut img = W::new(true, false);
    img.s(0x0008_1150, b"UI", "1.2.840.10008.5.1.4.1.1.2")
        .s(0x0008_1155, b"UI", "1.2.3.4")
        .s(0x0008_1160, b"IS", "2");
    let mut series = W::new(true, false);
    series
        .seq(0x0008_1140, &[img])
        .s(0x0020_000E, b"UI", "1.2.3");
    let mut w = W::new(true, false);
    w.s(0x0008_0016, b"UI", gsps::GSPS_SOP_CLASS)
        .s(0x0008_0018, b"UI", "9.9.9")
        .s(0x0008_0060, b"CS", "PR")
        .seq(0x0008_1115, &[series])
        .s(0x0070_0080, b"CS", "MINE")
        .s(0x0070_0081, b"LO", "A test state")
        .s(0x0070_0082, b"DA", "20261001")
        .s(0x0070_0083, b"TM", "120000");
    extra(&mut w);
    file(EXPLICIT, &w.out)
}

/// References, layers, annotations, spatial transform and displayed area of a state.
#[test]
fn gsps_parse() {
    use gsps::{GraphicKind, Justify, PresentationState, Shape, SizeMode, Units};
    let f = gsps_file(|w| {
        let graphic = |kind: &str, units: &str, pts: &[f32], filled: bool| {
            let mut g = W::new(true, false);
            g.s(0x0070_0005, b"CS", units)
                .us(0x0070_0020, &[2])
                .us(0x0070_0021, &[(pts.len() / 2) as u16])
                .raw(0x0070_0022, b"FL", &fl(pts))
                .s(0x0070_0023, b"CS", kind)
                .s(0x0070_0024, b"CS", if filled { "Y" } else { "N" });
            g
        };
        let mut text = W::new(true, false);
        text.s(0x0070_0003, b"CS", "DISPLAY")
            .s(0x0070_0004, b"CS", "PIXEL")
            .s(0x0070_0006, b"ST", "Lesion\r\nsecond line")
            .raw(0x0070_0010, b"FL", &fl(&[0.1, 0.1]))
            .raw(0x0070_0011, b"FL", &fl(&[0.4, 0.2]))
            .s(0x0070_0012, b"CS", "CENTER")
            .raw(0x0070_0014, b"FL", &fl(&[5.0, 6.0]))
            .s(0x0070_0015, b"CS", "Y");
        let mut ann = W::new(true, false);
        ann.s(0x0070_0002, b"CS", "TOP")
            .seq(0x0070_0008, &[text])
            .seq(
                0x0070_0009,
                &[
                    graphic(
                        "POLYLINE",
                        "PIXEL",
                        &[0.0, 0.0, 4.0, 0.0, 4.0, 3.0, 0.0, 0.0],
                        true,
                    ),
                    graphic("CIRCLE", "PIXEL", &[10.0, 10.0, 13.0, 14.0], false),
                    graphic("POINT", "DISPLAY", &[0.5, 0.5], false),
                    graphic(
                        "ELLIPSE",
                        "PIXEL",
                        &[0.0, 0.0, 8.0, 0.0, 4.0, -1.0, 4.0, 1.0],
                        false,
                    ),
                    // Too few points: dropped.
                    graphic("ELLIPSE", "PIXEL", &[0.0, 0.0, 8.0, 0.0], false),
                    graphic(
                        "INTERPOLATED",
                        "PIXEL",
                        &[0.0, 0.0, 5.0, 5.0, 10.0, 0.0],
                        false,
                    ),
                ],
            );
        let mut ann2 = W::new(true, false);
        ann2.s(0x0070_0002, b"CS", "BOTTOM");
        let layer = |name: &str, order: &str, lab: Option<[u16; 3]>, grey: Option<u16>| {
            let mut l = W::new(true, false);
            l.s(0x0070_0002, b"CS", name).s(0x0070_0062, b"IS", order);
            if let Some(lab) = lab {
                l.us(0x0070_0401, &lab);
            }
            if let Some(g) = grey {
                l.us(0x0070_0066, &[g]);
            }
            l
        };
        w.seq(0x0070_0001, &[ann, ann2])
            .s(0x0070_0041, b"CS", "Y")
            .us(0x0070_0042, &[90])
            .seq(
                0x0070_0060,
                &[
                    layer("TOP", "2", Some([65535, 32896, 32896]), None),
                    layer("BOTTOM", "1", None, Some(32768)),
                ],
            );
        let mut area = W::new(true, false);
        area.raw(0x0070_0052, b"SL", &sl(&[512, 512]))
            .raw(0x0070_0053, b"SL", &sl(&[257, 1]))
            .s(0x0070_0100, b"CS", "MAGNIFY")
            .raw(0x0070_0103, b"FL", &fl(&[2.0]));
        w.seq(0x0070_005A, &[area]);
        let mut voi = W::new(true, false);
        voi.s(0x0028_1050, b"DS", "100").s(0x0028_1051, b"DS", "50");
        w.seq(0x0028_3110, &[voi]).s(0x2050_0020, b"CS", "INVERSE");
    });
    let ps = PresentationState::parse(&f).unwrap();
    assert_eq!(ps.sop_uid, "9.9.9");
    assert_eq!(ps.label, "MINE");
    assert_eq!(ps.description.as_deref(), Some("A test state"));
    assert_eq!(ps.created, "20261001120000");
    assert!(ps.refers_to("1.2.3.4", 1));
    assert!(!ps.refers_to("1.2.3.4", 0));
    assert!(!ps.refers_to("1.2.3.5", 1));
    assert_eq!(ps.shape, Some(Shape::Inverse));
    assert_eq!(ps.rotation_flip(), (1, true));
    // Parts without references of their own apply to every referenced image.
    assert!(matches!(
        ps.voi_for("1.2.3.4", 1),
        Some(display::Voi::Window { center, width, .. }) if *center == 100.0 && *width == 50.0
    ));
    let a = ps.area_for("1.2.3.4", 1).unwrap();
    assert_eq!(a.stored_rect(), (256.0, 0.0, 512.0, 512.0));
    assert_eq!(a.mode, SizeMode::Magnify(2.0));
    // Layer order: BOTTOM (1) first.
    let anns = ps.annotations_for("1.2.3.4", 1);
    assert_eq!(
        anns.iter().map(|a| a.layer.as_str()).collect::<Vec<_>>(),
        ["BOTTOM", "TOP"]
    );
    assert_eq!(ps.layer("BOTTOM").unwrap().srgb(), Some([128; 3]));
    let white = ps.layer("TOP").unwrap().srgb().unwrap();
    assert!(white.iter().all(|&c| c >= 254), "{white:?}");
    let top = anns[1];
    let kinds: Vec<_> = top.graphics.iter().map(|g| g.kind).collect();
    assert_eq!(
        kinds,
        [
            GraphicKind::Polyline,
            GraphicKind::Circle,
            GraphicKind::Point,
            GraphicKind::Ellipse,
            GraphicKind::Interpolated
        ]
    );
    assert!(top.graphics[0].filled);
    assert_eq!(top.graphics[2].units, Units::Display);
    let t = &top.texts[0];
    assert_eq!(t.lines, ["Lesion", "second line"]);
    assert_eq!(t.justify, Justify::Center);
    let (u, tl, br) = t.bbox.unwrap();
    assert_eq!(u, Units::Display);
    assert!((tl.0 - 0.1).abs() < 1e-6 && (br.1 - 0.2).abs() < 1e-6);
    assert_eq!(t.anchor, Some((Units::Pixel, (5.0, 6.0), true)));
    // Other classes are refused.
    assert!(matches!(
        PresentationState::parse(&tiny(|_| {})),
        Err(Error::Unsupported(_))
    ));
}

/// Graphic outlines: closed polylines, circles and rotated ellipses as rings, splines through
/// their points.
#[test]
fn gsps_outlines() {
    use gsps::{Graphic, GraphicKind, Units};
    let g = |kind, points: &[(f64, f64)]| Graphic {
        units: Units::Pixel,
        kind,
        points: points.to_vec(),
        filled: false,
        style: Default::default(),
        compound: None,
    };
    let (p, closed) = g(
        GraphicKind::Polyline,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 0.0)],
    )
    .outline();
    assert!(closed);
    assert_eq!(p.len(), 3);
    let (_, closed) = g(GraphicKind::Polyline, &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]).outline();
    assert!(!closed);
    let (p, closed) = g(GraphicKind::Circle, &[(10.0, 10.0), (13.0, 14.0)]).outline();
    assert!(closed);
    assert!(
        p.iter()
            .all(|q| ((q.0 - 10.0).hypot(q.1 - 10.0) - 5.0).abs() < 1e-9)
    );
    // Major axis along the diagonal (half-length 2√2), minor half-length √2 / 2.
    let (p, closed) = g(
        GraphicKind::Ellipse,
        &[(0.0, 0.0), (4.0, 4.0), (1.5, 2.5), (2.5, 1.5)],
    )
    .outline();
    assert!(closed);
    for q in &p {
        let (x, y) = (q.0 - 2.0, q.1 - 2.0);
        let (u, v) = ((x + y) / 2f64.sqrt(), (y - x) / 2f64.sqrt());
        let e = u * u / 8.0 + v * v / 0.5;
        assert!((e - 1.0).abs() < 1e-9, "{q:?}");
    }
    let pts = [(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)];
    let (p, closed) = g(GraphicKind::Interpolated, &pts).outline();
    assert!(!closed);
    assert_eq!(p.len(), 2 * 12 + 1);
    // Through every control point.
    for (i, q) in pts.iter().enumerate() {
        let r = p[i * 12];
        assert!((r.0 - q.0).abs() < 1e-9 && (r.1 - q.1).abs() < 1e-9);
    }
}

#[test]
fn cielab_colours() {
    use gsps::cielab_to_srgb;
    let enc = |l: f64, a: f64, b: f64| {
        cielab_to_srgb(
            (l / 100.0 * 65535.0).round() as u16,
            ((a + 128.0) / 255.0 * 65535.0).round() as u16,
            ((b + 128.0) / 255.0 * 65535.0).round() as u16,
        )
    };
    let near = |got: [u8; 3], want: [u8; 3]| {
        assert!(
            got.iter()
                .zip(want)
                .all(|(g, w)| (*g as i32 - w as i32).abs() <= 2),
            "{got:?} != {want:?}"
        )
    };
    near(enc(100.0, 0.0, 0.0), [255, 255, 255]);
    near(enc(0.0, 0.0, 0.0), [0, 0, 0]);
    // sRGB primaries in D50 Lab (Bradford).
    near(enc(54.29, 80.80, 69.89), [255, 0, 0]);
    near(enc(87.82, -79.29, 80.99), [0, 255, 0]);
    near(enc(29.57, 68.30, -112.03), [0, 0, 255]);
}

/// A 1×4 grey image, SOP Instance `1.2.3.4`, with `extra` elements.
fn row_image(signed: bool, pm: &str, px: &[u16], extra: impl FnOnce(&mut W)) -> Vec<u8> {
    let mut w = W::new(true, false);
    w.s(0x0008_0018, b"UI", "1.2.3.4")
        .s(0x0008_0060, b"CS", "CT");
    extra(&mut w);
    w.image(1, px.len() as u16, 1, pm, (16, 16, 15), signed)
        .ow(PIXEL_DATA, px);
    file(EXPLICIT, &w.out)
}

fn presented(img: &[u8], ps: &[u8]) -> ModalityFrame {
    let ps = gsps::PresentationState::parse(ps).unwrap();
    let d = Dicom::parse(img).unwrap();
    d.modality_frame(img, 0, None).unwrap().presented(&ps, 1)
}

fn grey16(m: &ModalityFrame) -> Vec<u16> {
    match m.render_default().unwrap().pixels {
        Rendered::Gray16(v) => v,
        p => panic!("{p:?}"),
    }
}

/// A state's pipeline replaces the image's: its modality transform (identity when absent),
/// its VOI (identity over the possible values when absent), its shape (MONOCHROME1 no
/// longer inverts on its own), its Presentation LUT table.
#[test]
fn gsps_display_pipeline() {
    use display::{Voi, VoiSource};
    // MONOCHROME1 with its own rescale and window.
    let img = row_image(false, "MONOCHROME1", &[0, 100, 200, 300], |w| {
        w.s(0x0028_1052, b"DS", "-10")
            .s(0x0028_1053, b"DS", "2")
            .s(0x0028_1050, b"DS", "0")
            .s(0x0028_1051, b"DS", "10");
    });
    let d = Dicom::parse(&img).unwrap();
    let plain = d.modality_frame(&img, 0, None).unwrap();
    assert_eq!(plain.sop_instance_uid(), Some("1.2.3.4"));
    assert!(plain.inverted());
    assert_eq!(plain.value_at(1, 0), Some(190.0));
    // Window 100..200 (LINEAR_EXACT), identity modality, IDENTITY shape.
    let window = |w: &mut W, c: &str, wd: &str| {
        let mut voi = W::new(true, false);
        voi.s(0x0028_1050, b"DS", c).s(0x0028_1051, b"DS", wd).s(
            0x0028_1056,
            b"CS",
            "LINEAR_EXACT",
        );
        w.seq(0x0028_3110, &[voi]);
    };
    let ps = gsps_file(|w| {
        window(w, "150", "100");
        w.s(0x2050_0020, b"CS", "IDENTITY");
    });
    let m = presented(&img, &ps);
    assert_eq!(m.presentation(), Some("9.9.9"));
    assert!(!m.inverted());
    assert_eq!(m.value_at(1, 0), Some(100.0));
    assert_eq!(grey16(&m), [0, 0, 65535, 65535]);
    // The state's rescale; no VOI: identity over the possible values (0..65535 → −10..131060).
    let ps = gsps_file(|w| {
        w.s(0x0028_1052, b"DS", "-10")
            .s(0x0028_1053, b"DS", "2")
            .s(0x0028_1054, b"LO", "HU")
            .s(0x2050_0020, b"CS", "INVERSE");
    });
    let m = presented(&img, &ps);
    assert!(m.inverted());
    assert_eq!(m.unit(), Some("HU"));
    assert_eq!(m.value_at(3, 0), Some(590.0));
    let (voi, src) = m.default_voi();
    assert_eq!(src, VoiSource::Window);
    let Voi::Window { center, width, .. } = voi else {
        panic!("{voi:?}")
    };
    assert_eq!(
        (center - width / 2.0, center + width / 2.0),
        (-10.0, 131060.0)
    );
    let g = grey16(&m);
    assert_eq!(g[0], 65535);
    assert!((g[3] as i32 - (65535 - 300)).abs() <= 1, "{g:?}");
    // A Presentation LUT table, reversing, after the window.
    let ps = gsps_file(|w| {
        window(w, "150", "100");
        let data: Vec<u16> = (0..256).map(|i| 255 - i).collect();
        let mut plut = W::new(true, false);
        plut.us(0x0028_3002, &[256, 0, 8]).ow(0x0028_3006, &data);
        w.seq(0x2050_0010, &[plut]);
    });
    let m = presented(&img, &ps);
    assert!(m.has_presentation_lut());
    assert!(!m.inverted());
    assert_eq!(grey16(&m), [65535, 65535, 0, 0]);
    assert_eq!(
        m.display_values_f32(&m.default_voi().0),
        Some(vec![1.0, 1.0, 0.0, 0.0])
    );
}

/// LUT descriptors' first values (US) read as signed for signed input: a Modality LUT on a
/// signed image, a VOI LUT on modality values that can be negative.
#[test]
fn gsps_signed_luts() {
    let px = [-2048i16, -2047, -2046, -2045].map(|v| v as u16);
    let img = row_image(true, "MONOCHROME2", &px, |_| {});
    let lut = |data: &[u16]| {
        let mut l = W::new(true, false);
        l.us(0x0028_3002, &[4, 63488, 16]).ow(0x0028_3006, data);
        l
    };
    let ps = gsps_file(|w| {
        w.seq(0x0028_3000, &[lut(&[10, 20, 30, 40])]);
    });
    let m = presented(&img, &ps);
    assert_eq!(
        (0..4)
            .map(|x| m.value_at(x, 0).unwrap())
            .collect::<Vec<_>>(),
        [10.0, 20.0, 30.0, 40.0]
    );
    let ps = gsps_file(|w| {
        let mut voi = W::new(true, false);
        voi.seq(0x0028_3010, &[lut(&[0, 1000, 2000, 65535])]);
        w.seq(0x0028_3110, &[voi]);
    });
    let m = presented(&img, &ps);
    assert_eq!(m.default_voi().1, display::VoiSource::Lut);
    let f = m.display_values_f32(&m.default_voi().0).unwrap();
    let want = [0.0, 1000.0 / 65535.0, 2000.0 / 65535.0, 1.0];
    assert!(
        f.iter()
            .zip(want)
            .all(|(a, b)| (*a as f64 - b).abs() < 1e-6),
        "{f:?}"
    );
}

/// The image's own Presentation LUT Shape: INVERSE inverts MONOCHROME2.
#[test]
fn image_presentation_lut_shape() {
    let img = row_image(false, "MONOCHROME2", &[0, 100, 200, 300], |w| {
        w.s(0x2050_0020, b"CS", "INVERSE");
    });
    let d = Dicom::parse(&img).unwrap();
    let m = d.modality_frame(&img, 0, None).unwrap();
    assert!(m.inverted());
    let g = grey16(&m);
    assert_eq!(g[0], 65535);
    assert_eq!(g[3], 0);
    let img = row_image(false, "MONOCHROME2", &[0, 100, 200, 300], |w| {
        w.s(0x2050_0020, b"CS", "IDENTITY");
    });
    let d = Dicom::parse(&img).unwrap();
    assert!(!d.modality_frame(&img, 0, None).unwrap().inverted());
}

/// Writes overlay group `g`: `rows`×`cols` at 1-based `origin` (row, column), packed `bits`.
fn overlay(w: &mut W, g: u32, rows: u16, cols: u16, origin: (i16, i16), bits: &[u8]) {
    let o = [origin.0.to_le_bytes(), origin.1.to_le_bytes()].concat();
    w.us(g << 16 | 0x0010, &[rows])
        .us(g << 16 | 0x0011, &[cols])
        .s(g << 16 | 0x0040, b"CS", "G")
        .raw(g << 16 | 0x0050, b"SS", &o)
        .us(g << 16 | 0x0100, &[1])
        .us(g << 16 | 0x0102, &[0])
        .raw(g << 16 | 0x3000, b"OB", bits);
}

/// A graphic layer item with a recommended grey.
fn layer_item(name: &str, order: &str, grey: u16) -> W {
    let mut l = W::new(true, false);
    l.s(0x0070_0002, b"CS", name)
        .s(0x0070_0062, b"IS", order)
        .us(0x0070_0066, &[grey]);
    l
}

/// Under a state: its shutter replaces the image's, with its colour; only the overlays it
/// activates show, its own plane of a group in place of the image's, inked in their layers'
/// grey and stacked in layer order.
#[test]
fn gsps_shutter_and_overlay_activation() {
    use crate::graphics::{INK, SHUTTER};
    let img = graphics_file(
        1,
        |w| {
            // The image's own shutter hides columns 1 and 2.
            w.s(0x0018_1600, b"CS", "RECTANGULAR")
                .s(0x0018_1602, b"IS", "3")
                .s(0x0018_1604, b"IS", "8")
                .s(0x0018_1606, b"IS", "1")
                .s(0x0018_1608, b"IS", "6");
            overlay(w, 0x6000, 2, 2, (1, 1), &[0b1111]);
            overlay(w, 0x6002, 1, 1, (1, 1), &[0b1]);
            overlay(w, 0x6004, 1, 1, (6, 8), &[0b1]);
        },
        None,
    );
    let d = Dicom::parse(&img).unwrap();
    let plain = d.modality_frame(&img, 0, None).unwrap();
    let g = plain.graphics().unwrap();
    assert_eq!((g.overlays, g.inks.len()), (3, 1));
    assert_eq!(g.at(0, 0) & SHUTTER, SHUTTER);
    let ps = gsps_file(|w| {
        w.s(0x0018_1600, b"CS", "RECTANGULAR")
            .s(0x0018_1602, b"IS", "1")
            .s(0x0018_1604, b"IS", "7")
            .s(0x0018_1606, b"IS", "1")
            .s(0x0018_1608, b"IS", "6")
            .us(0x0018_1622, &[1234])
            // Green (L 87.82, a −79.29, b 80.99).
            .us(0x0018_1624, &[57553, 12519, 53710]);
        overlay(w, 0x6002, 2, 2, (1, 2), &[0b1111]);
        w.s(0x6000_1001, b"CS", "TOP")
            .s(0x6002_1001, b"CS", "MID")
            .seq(
                0x0070_0060,
                &[layer_item("TOP", "2", 65535), layer_item("MID", "1", 32768)],
            );
    });
    let m = presented(&img, &ps);
    let g = m.graphics().unwrap().clone();
    assert!(g.shutter);
    assert_eq!(g.overlays, 2);
    // MID (order 1) first: ink 1 grey, ink 2 white.
    assert_eq!(g.inks, [[128; 3], [255; 3]]);
    let ink = |x, y| (g.at(x, y) & INK) >> 1;
    // 6000 alone, 6000 over the state's 6002, the state's 6002 alone.
    assert_eq!((ink(0, 0), ink(1, 0), ink(2, 0), ink(2, 1)), (2, 2, 1, 1));
    assert_eq!((ink(0, 1), ink(1, 1)), (2, 2));
    // The image's 6002 (only pixel 0, 0) is replaced, 6004 is not activated.
    assert_eq!((ink(3, 0), ink(7, 5)), (0, 0));
    // The state's shutter: column 8 only; the image's (columns 1, 2) is gone.
    assert_eq!(g.at(7, 0) & SHUTTER, SHUTTER);
    assert_eq!((g.at(0, 3) & SHUTTER, g.at(1, 3) & SHUTTER), (0, 0));
    let green = g.shutter_rgb.unwrap();
    assert!(
        green[0] <= 2 && green[1] >= 253 && green[2] <= 2,
        "{green:?}"
    );
    assert_eq!(g.shutter_grey, 1234);
    // Burnt into grey output: the shutter colour's luma, the inks' grey.
    let p = grey16(&m);
    let luma = (0.2126 * f64::from(green[0])
        + 0.7152 * f64::from(green[1])
        + 0.0722 * f64::from(green[2]))
        * 257.0;
    assert_eq!(p[7], luma.round() as u16);
    assert_eq!((p[0], p[2]), (65535, 128 * 257));
    // Without colour the Shutter Presentation Value.
    let ps = gsps_file(|w| {
        w.s(0x0018_1600, b"CS", "CIRCULAR")
            .s(0x0018_1610, b"IS", "3\\4")
            .s(0x0018_1612, b"IS", "2")
            .us(0x0018_1622, &[1234]);
    });
    let m = presented(&img, &ps);
    let g = m.graphics().unwrap();
    assert_eq!((g.overlays, g.shutter_rgb), (0, None));
    assert_eq!(grey16(&m)[0], 1234);
    // A state with neither shows neither, whatever the image has.
    let m = presented(&img, &gsps_file(|_| {}));
    assert!(m.graphics().is_none());
}

/// A bitmap shutter: the set bits of the state's overlay of Shutter Overlay Group are
/// shuttered; that overlay is not shown as one.
#[test]
fn gsps_bitmap_shutter() {
    use crate::graphics::SHUTTER;
    let img = graphics_file(1, |_| {}, None);
    let ps = gsps_file(|w| {
        w.s(0x0018_1600, b"CS", "BITMAP")
            .us(0x0018_1623, &[0x6006])
            .us(0x0018_1622, &[65535]);
        overlay(
            w,
            0x6006,
            6,
            8,
            (1, 1),
            &[0b0000_0101, 0, 0, 0, 0, 0b1000_0000],
        );
    });
    let m = presented(&img, &ps);
    let g = m.graphics().unwrap();
    assert_eq!(g.overlays, 0);
    let shut: Vec<u32> = (0..48)
        .filter(|&i| g.at(i % 8, i / 8) & SHUTTER != 0)
        .collect();
    assert_eq!(shut, [0, 2, 47]);
    let p = grey16(&m);
    assert_eq!((p[0], p[2], p[47]), (65535, 65535, 65535));
    assert_ne!(p[1], 65535);
    // A bitmap shutter naming a missing overlay shutters nothing.
    let ps = gsps_file(|w| {
        w.s(0x0018_1600, b"CS", "BITMAP").us(0x0018_1623, &[0x6008]);
    });
    assert!(presented(&img, &ps).graphics().is_none());
}

/// Compound graphics: parsed with their attributes, their plain renderings (same Compound
/// Graphic Instance ID) dropped, and drawn as the standard describes; line, fill and text
/// styles; the Presentation Pixel Aspect Ratio.
#[test]
fn gsps_compound_graphics() {
    use gsps::{CompoundKind, Justify, PresentationState, Sizes, TickAlign, Units, VAlign};
    let compound = |kind: &str, pts: &[f32], extra: &dyn Fn(&mut W)| {
        let mut c = W::new(true, false);
        c.s(0x0070_0282, b"CS", "PIXEL")
            .s(0x0070_0294, b"CS", kind)
            .raw(0x0070_0022, b"FL", &fl(pts));
        extra(&mut c);
        c
    };
    // sRGB red (L 54.29, a 80.80, b 69.89).
    let lab_red = [35579u16, 53661, 50857];
    let f = gsps_file(|w| {
        let mut style = W::new(true, false);
        style
            .us(0x0070_0251, &lab_red)
            .raw(0x0070_0253, b"FL", &fl(&[2.5]))
            .s(0x0070_0254, b"CS", "DASHED");
        let mut fill = W::new(true, false);
        fill.raw(0x0070_0284, b"FL", &fl(&[0.25]));
        let arrow = compound("ARROW", &[10.0, 10.0, 20.0, 10.0], &|c| {
            c.raw(0x0070_0226, b"UL", &7u32.to_le_bytes())
                .seq(0x0070_0232, std::slice::from_ref(&style));
        });
        let mut tick = W::new(true, false);
        tick.raw(0x0070_0288, b"FL", &fl(&[0.5]))
            .s(0x0070_0289, b"SH", "5");
        let axis = compound("AXIS", &[0.0, 0.0, 10.0, 0.0], &|c| {
            c.seq(0x0070_0287, std::slice::from_ref(&tick))
                .s(0x0070_0274, b"CS", "TOP")
                .s(0x0070_0279, b"CS", "BOTTOM");
        });
        let cross = compound("CROSSHAIR", &[50.0, 50.0], &|c| {
            c.raw(0x0070_0261, b"FL", &fl(&[4.0]))
                .raw(0x0070_0262, b"FL", &fl(&[20.0]));
        });
        let rect = compound("RECTANGLE", &[0.0, 0.0, 4.0, 2.0], &|c| {
            c.raw(0x0070_0230, b"FD", &90f64.to_le_bytes())
                .s(0x0070_0024, b"CS", "Y")
                .seq(0x0070_0233, std::slice::from_ref(&fill));
        });
        let infinite = compound("INFINITELINE", &[0.0, 0.0, 10.0, 0.0], &|c| {
            c.raw(0x0070_0261, b"FL", &fl(&[2.0]));
        });
        let ruler = compound("RULER", &[0.0, 0.0, 10.0, 0.0], &|_| {});
        // The arrow's plain rendering, and a graphic of a compound the state lacks.
        let plain = |id: u32| {
            let mut g = W::new(true, false);
            g.s(0x0070_0005, b"CS", "PIXEL")
                .us(0x0070_0020, &[2])
                .us(0x0070_0021, &[2])
                .raw(0x0070_0022, b"FL", &fl(&[10.0, 10.0, 20.0, 10.0]))
                .s(0x0070_0023, b"CS", "POLYLINE")
                .raw(0x0070_0226, b"UL", &id.to_le_bytes());
            g
        };
        let mut text_style = W::new(true, false);
        text_style
            .us(0x0070_0241, &lab_red)
            .s(0x0070_0242, b"CS", "RIGHT")
            .s(0x0070_0243, b"CS", "BOTTOM");
        let mut text = W::new(true, false);
        text.s(0x0070_0003, b"CS", "PIXEL")
            .s(0x0070_0006, b"ST", "styled")
            .raw(0x0070_0010, b"FL", &fl(&[1.0, 1.0]))
            .raw(0x0070_0011, b"FL", &fl(&[5.0, 3.0]))
            .s(0x0070_0012, b"CS", "LEFT")
            .seq(0x0070_0231, &[text_style]);
        let mut label = W::new(true, false);
        label
            .s(0x0070_0004, b"CS", "PIXEL")
            .s(0x0070_0006, b"ST", "arrow")
            .raw(0x0070_0014, b"FL", &fl(&[20.0, 10.0]))
            .raw(0x0070_0226, b"UL", &7u32.to_le_bytes());
        let mut ann = W::new(true, false);
        ann.s(0x0070_0002, b"CS", "L")
            .seq(0x0070_0008, &[text, label])
            .seq(0x0070_0009, &[plain(7), plain(8)])
            .seq(0x0070_0209, &[arrow, axis, cross, rect, infinite, ruler]);
        w.seq(0x0070_0001, &[ann])
            .seq(0x0070_0060, &[layer_item("L", "1", 65535)]);
        let mut area = W::new(true, false);
        area.raw(0x0070_0052, b"SL", &sl(&[1, 1]))
            .raw(0x0070_0053, b"SL", &sl(&[8, 6]))
            .s(0x0070_0102, b"IS", "2\\1");
        w.seq(0x0070_005A, &[area]);
    });
    let ps = PresentationState::parse(&f).unwrap();
    assert_eq!(ps.areas[0].aspect, 2.0);
    let a = &ps.annotations[0];
    // The arrow's polyline and label give way to it; graphic 8 has no compound and stays.
    assert_eq!(a.graphics.len(), 1);
    assert_eq!(a.graphics[0].compound, Some(8));
    assert_eq!(a.texts.len(), 1);
    let t = &a.texts[0];
    assert_eq!((t.justify, t.valign), (Justify::Right, VAlign::Bottom));
    let red = t.colour.unwrap();
    assert!(red[0] >= 250 && red[1] <= 5 && red[2] <= 5, "{red:?}");
    let kinds: Vec<_> = a.compounds.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            CompoundKind::Arrow,
            CompoundKind::Axis,
            CompoundKind::Crosshair,
            CompoundKind::Rectangle,
            CompoundKind::InfiniteLine,
            CompoundKind::Ruler
        ]
    );
    let [arrow, axis, cross, rect, infinite, ruler] = &a.compounds[..] else {
        unreachable!()
    };
    assert_eq!((arrow.units, arrow.id), (Units::Pixel, Some(7)));
    assert_eq!(arrow.style.colour, t.colour);
    assert_eq!(
        (arrow.style.thickness, arrow.style.dashed),
        (Some(2.5), true)
    );
    assert_eq!(rect.style.opacity, Some(0.25));
    assert!(rect.filled);
    let s = Sizes {
        display: 2.0,
        reach: 1000.0,
        head: 3.0,
    };
    let near = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9;
    // Arrow: the shaft from the foot to the anchor, the head's tip on the anchor.
    let (st, _) = arrow.draw(&s);
    assert_eq!(st.len(), 2);
    assert!(near(st[0].points[1], (10.0, 10.0)));
    assert!(st[1].filled && near(st[1].points[1], (10.0, 10.0)));
    // Axis: one tick at the middle, above the line (rows grow down); its label below.
    assert_eq!(axis.tick_align, TickAlign::Top);
    let (st, labels) = axis.draw(&s);
    assert_eq!(st.len(), 2);
    assert!(near(st[1].points[0], (5.0, 0.0)) && near(st[1].points[1], (5.0, -3.0)));
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "5");
    assert!(labels[0].below && labels[0].at.1 > 0.0);
    // Crosshair: four arms from the gap (4 display units across: radius 4) to the
    // diameter of visibility (20 across: radius 20).
    let (st, _) = cross.draw(&s);
    assert_eq!(st.len(), 4);
    assert!(near(st[0].points[0], (54.0, 50.0)) && near(st[0].points[1], (70.0, 50.0)));
    // Rectangle turned 90° counter-clockwise about its centre (2, 1): the right end goes up.
    let (st, _) = rect.draw(&s);
    assert!(st[0].filled);
    assert!(near(st[0].points[1], (1.0, -1.0)), "{:?}", st[0].points);
    // Infinite line: out to the reach both ways, the gap (2 across: radius 2) left out.
    let (st, _) = infinite.draw(&s);
    assert_eq!(st.len(), 2);
    assert!(near(st[0].points[0], (-995.0, 0.0)) && near(st[0].points[1], (3.0, 0.0)));
    assert!(near(st[1].points[0], (7.0, 0.0)) && near(st[1].points[1], (1005.0, 0.0)));
    // A ruler without ticks: eleven, centred on the line.
    let (st, labels) = ruler.draw(&s);
    assert_eq!((st.len(), labels.len()), (12, 0));
    assert!(near(st[11].points[0], (10.0, 1.5)) && near(st[11].points[1], (10.0, -1.5)));
}

#[test]
fn jpeg2000_transform_flag() {
    // SOC, SIZ (a 3-component stub), COD with and without the transform, then SOT.
    let stream = |mct: u8| {
        let mut d = vec![0xff, 0x4f, 0xff, 0x51, 0x00, 0x04, 0x00, 0x00];
        d.extend([
            0xff, 0x52, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x01, mct, 0x05, 0x04, 0x04, 0x00, 0x00,
        ]);
        d.extend([0xff, 0x90, 0x00, 0x0a]);
        d
    };
    assert_eq!(codec::codestream_mct(&stream(0)), Some(0));
    assert_eq!(codec::codestream_mct(&stream(1)), Some(1));
    // No COD before the first tile, a JP2 file, a cut header.
    assert_eq!(
        codec::codestream_mct(&[0xff, 0x4f, 0xff, 0x90, 0x00, 0x0a]),
        None
    );
    assert_eq!(codec::codestream_mct(b"\0\0\0\x0cjP  "), None);
    assert_eq!(codec::codestream_mct(&stream(0)[..10]), None);
}

/// A `side`² 12-bit grey image whose every overlay group (0x6000..=0x601E) is a retired
/// plane embedded in bit 12 + k % 4 of the cells, 1-based origin (1 + k, 1 + k) for the `k`th
/// group; cell `i`'s overlay bits are `i % 16`.
fn embedded_overlays(side: u16) -> Vec<u8> {
    let mut w = W::new(true, false);
    w.image(side, side, 1, "MONOCHROME2", (16, 12, 11), false);
    for k in 0..16u32 {
        let g = 0x6000 + 2 * k;
        let o = (1 + k as i16).to_le_bytes();
        w.us(g << 16 | 0x0010, &[side])
            .us(g << 16 | 0x0011, &[side])
            .s(g << 16 | 0x0040, b"CS", "G")
            .raw(g << 16 | 0x0050, b"SS", &[o[0], o[1], o[0], o[1]])
            .us(g << 16 | 0x0100, &[16])
            .us(g << 16 | 0x0102, &[12 + (k % 4) as u16]);
    }
    let n = usize::from(side) * usize::from(side);
    let px: Vec<u16> = (0..n)
        .map(|i| (i % 4096) as u16 | ((i % 16) as u16) << 12)
        .collect();
    w.ow(PIXEL_DATA, &px);
    file(EXPLICIT, &w.out)
}

#[test]
fn embedded_overlays_in_one_pass() {
    use crate::graphics::OVERLAY;
    let side = 9u16;
    let f = embedded_overlays(side);
    let d = Dicom::parse(&f).unwrap();
    let m = d.modality_frame(&f, 0, None).unwrap();
    let g = m.graphics().unwrap();
    assert_eq!(g.overlays, 16);
    let s = u32::from(side);
    for y in 0..s {
        for x in 0..s {
            // Group k shows cell (y - k, x - k) when its bit is set there.
            let on = (0..16u32).any(|k| {
                let (cy, cx) = (y as i64 - k as i64, x as i64 - k as i64);
                cy >= 0 && cx >= 0 && {
                    let i = cy as u32 * s + cx as u32;
                    (i % 16) >> (k % 4) & 1 == 1
                }
            });
            assert_eq!(g.at(x, y) & OVERLAY != 0, on, "({x}, {y})");
        }
    }
    for (k, p) in g.planes.iter().enumerate() {
        assert_eq!(p.group, 0x6000 + 2 * k as u16);
    }
}

#[test]
fn hostile_sixteen_embedded_overlays() {
    // Every overlay group embedded in a 4096² frame: each read all 16M cells again.
    let f = embedded_overlays(4096);
    let d = Dicom::parse(&f).unwrap();
    let frame = d.decode_frame(&f, 0, None).unwrap();
    let t = std::time::Instant::now();
    let m = d.modality_of(&f, 0, frame).unwrap();
    let el = t.elapsed();
    assert!(el < std::time::Duration::from_secs(5), "{el:?}");
    assert_eq!(m.graphics().unwrap().overlays, 16);
}

/// A veterinary image (Anatomical Orientation Type QUADRUPED) names its directions as PS3.3
/// C.7.6.1.1.1 does, on the axes of C.7.6.2.1.1, by the body part: Le/Rt, D/V, Cr/Cd on the
/// trunk, Pa below the carpus; Patient Orientation in the quadruped codes.
#[test]
fn quadruped_orientation_labels() {
    let axes = |f: &[u8]| {
        let d = Dicom::parse(f).unwrap();
        d.modality_frame(f, 0, None)
            .unwrap()
            .patient_axes()
            .copied()
            .unwrap()
    };
    // A transverse slice of the trunk: rows to the animal's left, columns ventral.
    let f = tiny(|w| {
        w.s(0x0010_2210, b"CS", "QUADRUPED")
            .s(0x0018_0015, b"CS", "THORAX")
            .s(0x0020_0037, b"DS", "1\\0\\0\\0\\-1\\0");
    });
    let a = axes(&f);
    assert_eq!(a.anatomy, crate::Anatomy::Quadruped(crate::Region::Trunk));
    assert_eq!(
        [
            a.label(1.0, 0.0),
            a.label(-1.0, 0.0),
            a.label(0.0, 1.0),
            a.label(0.0, -1.0)
        ],
        [
            Some("Le".into()),
            Some("Rt".into()),
            Some("V".into()),
            Some("D".into())
        ]
    );
    // A dorsal plane of the carpus: columns distal; palmar is ventral there.
    let f = tiny(|w| {
        w.s(0x0010_2210, b"CS", "QUADRUPED")
            .s(0x0018_0015, b"CS", "CARPUS")
            .s(0x0020_0037, b"DS", "1\\0\\0\\0\\0\\-1");
    });
    let a = axes(&f);
    assert_eq!(a.label(0.0, 1.0).unwrap(), "Di");
    assert_eq!(a.anatomy.label([0.0, -1.0, 0.0]).unwrap(), "Pa");
    // Patient Orientation in the quadruped codes.
    let f = tiny(|w| {
        w.s(0x0010_2210, b"CS", "QUADRUPED")
            .s(0x0020_0020, b"CS", "LE\\CR");
    });
    let a = axes(&f);
    assert!(a.from_letters);
    assert_eq!(a.label(1.0, 0.0).unwrap(), "Le");
    assert_eq!(a.label(0.0, -1.0).unwrap(), "Cd");
    // BIPED or absent: the human letters.
    let f = tiny(|w| {
        w.s(0x0010_2210, b"CS", "BIPED")
            .s(0x0020_0037, b"DS", "1\\0\\0\\0\\1\\0");
    });
    assert_eq!(axes(&f).label(0.0, 1.0).unwrap(), "P");
}
