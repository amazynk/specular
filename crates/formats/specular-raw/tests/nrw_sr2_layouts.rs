//! Two RAW layouts, as synthetic files: the Nikon Coolpix P NRW packing (12 bits
//! MSB-first over little-endian 32-bit words) and the Sony DSC-R1's big-endian samples in a
//! little-endian TIFF. Both were found against LibRaw on raw.pixls.us files
//! (`RAW_NIKON_P7000.NRW`, `_DSC1477.SR2`), where 98 % and 99.5 % of the samples differed.

use specular_raw::DecodeOptions;

/// A little-endian TIFF: IFD0 with Make and Model and one SubIFD holding the sensor strip.
fn tiff(make: &str, model: &str, w: u32, h: u32, bits: u16, strip: &[u8]) -> Vec<u8> {
    // Layout: header, IFD0, raw IFD, strings, strip.
    let ifd0_n = 3u16;
    let raw_n = 8u16;
    let ifd0 = 8u32;
    let raw = ifd0 + 2 + 12 * u32::from(ifd0_n) + 4;
    let strings = raw + 2 + 12 * u32::from(raw_n) + 4;
    let make_z = format!("{make}\0");
    let model_z = format!("{model}\0");
    let model_at = strings + make_z.len() as u32;
    let data_at = (model_at + model_z.len() as u32).next_multiple_of(16);
    let mut d = b"II*\0".to_vec();
    d.extend_from_slice(&ifd0.to_le_bytes());
    let ent = |d: &mut Vec<u8>, tag: u16, typ: u16, count: u32, v: u32| {
        d.extend_from_slice(&tag.to_le_bytes());
        d.extend_from_slice(&typ.to_le_bytes());
        d.extend_from_slice(&count.to_le_bytes());
        d.extend_from_slice(&v.to_le_bytes());
    };
    d.extend_from_slice(&ifd0_n.to_le_bytes());
    ent(&mut d, 0x010f, 2, make_z.len() as u32, strings);
    ent(&mut d, 0x0110, 2, model_z.len() as u32, model_at);
    ent(&mut d, 0x014a, 4, 1, raw);
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&raw_n.to_le_bytes());
    ent(&mut d, 0x00fe, 4, 1, 0);
    ent(&mut d, 0x0100, 4, 1, w);
    ent(&mut d, 0x0101, 4, 1, h);
    ent(&mut d, 0x0102, 3, 1, u32::from(bits));
    ent(&mut d, 0x0103, 3, 1, 1);
    ent(&mut d, 0x0106, 3, 1, 32803);
    ent(&mut d, 0x0111, 4, 1, data_at);
    ent(&mut d, 0x0117, 4, 1, strip.len() as u32);
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(make_z.as_bytes());
    d.extend_from_slice(model_z.as_bytes());
    d.resize(data_at as usize, 0);
    d.extend_from_slice(strip);
    d
}

fn samples(w: u32, h: u32, max: u32) -> Vec<u16> {
    (0..w * h)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 8) as u16 % max as u16)
        .collect()
}

#[test]
fn coolpix_p_nrw_packs_over_32_bit_words() {
    let (w, h) = (64u32, 6u32);
    let v = samples(w, h, 4096);
    // MSB-first bit stream, then each 32-bit word stored little-endian.
    let mut be = Vec::new();
    for p in v.chunks(2) {
        be.extend_from_slice(&[
            (p[0] >> 4) as u8,
            ((p[0] & 15) << 4 | p[1] >> 8) as u8,
            p[1] as u8,
        ]);
    }
    let strip: Vec<u8> = be
        .chunks(4)
        .flat_map(|c| [c[3], c[2], c[1], c[0]])
        .collect();
    let file = tiff("NIKON", "COOLPIX P7000", w, h, 12, &strip);
    let raw = specular_raw::parse(&file).unwrap();
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert_eq!(img.data, v);
    assert_eq!(img.white, 4095.0);
}

#[test]
fn sony_r1_samples_are_big_endian() {
    let (w, h) = (32u32, 4u32);
    let v = samples(w, h, 16384);
    let strip: Vec<u8> = v.iter().flat_map(|s| s.to_be_bytes()).collect();
    let file = tiff("SONY", "DSC-R1", w, h, 14, &strip);
    let raw = specular_raw::parse(&file).unwrap();
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert_eq!(img.data, v);
    // Another Sony body's uncompressed samples stay little-endian.
    let strip: Vec<u8> = v.iter().flat_map(|s| s.to_le_bytes()).collect();
    let file = tiff("SONY", "DSLR-A700", w, h, 14, &strip);
    let raw = specular_raw::parse(&file).unwrap();
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert_eq!(img.data, v);
}
