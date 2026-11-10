//! Hand-built hostile codestreams: header values at the edges of their ranges must give an
//! error or an image, never a panic (debug builds check arithmetic overflow).

use specular_jp2::{DecodeOptions, Parallelism};

fn seg(out: &mut Vec<u8>, marker: u16, body: &[u8]) {
    out.extend_from_slice(&marker.to_be_bytes());
    out.extend_from_slice(&u16::try_from(body.len() + 2).unwrap().to_be_bytes());
    out.extend_from_slice(body);
}

/// One 8-bit component, one tile at (`x0`, `y0`) of `tw` × `th`, no decomposition levels,
/// 64×64 code-blocks, one empty packet.
fn codestream(x0: u32, y0: u32, x1: u32, y1: u32, tw: u32, th: u32) -> Vec<u8> {
    codestream_with(x0, y0, x1, y1, tw, th, 1, 0, &[])
}

/// [`codestream`] with `nc` components subsampled by `sub`, `levels` decomposition levels and
/// user precinct sizes `precincts` (default when empty), all packets empty.
#[allow(clippy::too_many_arguments)]
fn codestream_with(
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    tw: u32,
    th: u32,
    sub: u8,
    levels: u8,
    precincts: &[u8],
) -> Vec<u8> {
    let mut cs = vec![0xFF, 0x4F];
    let mut siz = vec![0, 0];
    for v in [x1, y1, x0, y0, tw, th, x0, y0] {
        siz.extend_from_slice(&v.to_be_bytes());
    }
    siz.extend_from_slice(&1u16.to_be_bytes());
    siz.extend_from_slice(&[7, sub, sub]);
    seg(&mut cs, 0xFF51, &siz);
    let scod = u8::from(!precincts.is_empty());
    let mut cod = vec![scod, 0, 0, 1, 0, levels, 4, 4, 0, 1];
    cod.extend_from_slice(precincts);
    seg(&mut cs, 0xFF52, &cod);
    let mut qcd = vec![0x40];
    qcd.extend(std::iter::repeat_n(8 << 3, 1 + 3 * usize::from(levels)));
    seg(&mut cs, 0xFF5C, &qcd);
    let data = [0u8; 64];
    let psot = 12 + 2 + u32::try_from(data.len()).unwrap();
    let mut sot = 0u16.to_be_bytes().to_vec();
    sot.extend_from_slice(&psot.to_be_bytes());
    sot.extend_from_slice(&[0, 1]);
    seg(&mut cs, 0xFF90, &sot);
    cs.extend_from_slice(&[0xFF, 0x93]);
    cs.extend_from_slice(&data);
    cs.extend_from_slice(&[0xFF, 0xD9]);
    cs
}

/// A 20000 × 20000 grey codestream (400 MB of output, under the cap) of a few hundred bytes:
/// refused under a 64 MiB memory budget before the output is allocated.
#[test]
fn output_within_memory_budget() {
    let o = DecodeOptions {
        parallelism: Parallelism::Single,
        ..Default::default()
    };
    let cs = codestream(0, 0, 20_000, 20_000, 20_000, 20_000);
    let e =
        specular_core::budget::with_limit(64 << 20, || specular_jp2::decode(&cs, &o)).unwrap_err();
    let specular_jp2::Jp2Error::NeedsMemory(n) = e else {
        panic!("{e:?}")
    };
    assert!(n.needed >= 400_000_000 && n.needed > n.free, "{n:?}");
    // A 64 × 64 one fits.
    let cs = codestream(0, 0, 64, 64, 64, 64);
    assert!(specular_core::budget::with_limit(64 << 20, || specular_jp2::decode(&cs, &o)).is_ok());
}

#[test]
fn code_blocks_at_the_end_of_the_reference_grid() {
    // A 255×255 image in the last corner of the 32-bit reference grid: the last code-block
    // there ends past u32::MAX.
    let cs = codestream(0xFFFF_FF00, 0xFFFF_FF00, u32::MAX, u32::MAX, 0x100, 0x100);
    let img = specular_jp2::decode(
        &cs,
        &DecodeOptions {
            parallelism: Parallelism::Single,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((img.width, img.height), (255, 255));
}

#[test]
fn edge_of_grid_variants_do_not_panic() {
    let m = u32::MAX;
    let cases: Vec<Vec<u8>> = vec![
        codestream_with(0xFFFF_FF00, 0xFFFF_FF00, m, m, 0x100, 0x100, 1, 5, &[]),
        codestream_with(0xFFFF_FF00, 0xFFFF_FF00, m, m, 0x100, 0x100, 3, 5, &[]),
        codestream_with(
            0xFFFF_FF00,
            0xFFFF_FF00,
            m,
            m,
            0x100,
            0x100,
            1,
            3,
            &[0xFF; 4],
        ),
        codestream_with(0xFFFF_FF00, 0xFFFF_FF00, m, m, 0x100, 0x100, 1, 32, &[]),
        codestream_with(0xFFFF_FF00, 0xFFFF_FF00, m, m, m, m, 255, 32, &[]),
        codestream_with(0, 0, m, 1, m, 1, 1, 32, &[]),
        codestream_with(0xFFFF_FFF0, 0, m, 16, 16, 16, 1, 2, &[0x11, 0x11, 0x11]),
    ];
    for cs in &cases {
        // Whole images at the output cap (2^31 pixels) are valid but slow: regions only.
        let huge = specular_jp2::read_info(cs)
            .is_ok_and(|i| u64::from(i.width) * u64::from(i.height) > 1 << 22);
        for reduce in [0, 1, 5] {
            for region in [
                None,
                Some(specular_jp2::Region {
                    x: 1,
                    y: 1,
                    w: 3,
                    h: 3,
                }),
            ] {
                if huge && region.is_none() {
                    continue;
                }
                let _ = specular_jp2::decode(
                    cs,
                    &DecodeOptions {
                        reduce,
                        region,
                        parallelism: Parallelism::Single,
                        ..Default::default()
                    },
                );
            }
        }
        if !huge {
            let _ = specular_jp2::Decoder::new(cs).map(|d| d.decode_components_i32());
        }
    }
}
