//! Inputs found by `fuzz/` (cargo-fuzz), reduced to what triggered the bug.

use specular_webp::Animation;

/// RIFF chunks of a WebP file: (fourcc, payload).
fn chunks(file: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut i = 12;
    while i + 8 <= file.len() {
        let n = u32::from_le_bytes(file[i + 4..i + 8].try_into().unwrap()) as usize;
        out.push((file[i..i + 4].try_into().unwrap(), &file[i + 8..i + 8 + n]));
        i += 8 + n + (n & 1);
    }
    out
}

fn chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

fn u24(v: u32) -> [u8; 3] {
    [v as u8, (v >> 8) as u8, (v >> 16) as u8]
}

/// An ANMF frame with ALPH + VP8 whose VP8 bitstream is larger than the frame size in the
/// ANMF header indexed past the alpha plane (sized from the header) and panicked.
#[test]
fn anmf_alpha_frame_with_larger_vp8() {
    // 7x5 lossy image with raw (uncompressed) alpha.
    let still = include_bytes!("../fuzz/seeds/lossy_alpha_raw.webp");
    let parts = chunks(still);
    let alph = parts.iter().find(|c| &c.0 == b"ALPH").unwrap().1;
    let vp8 = parts.iter().find(|c| &c.0 == b"VP8 ").unwrap().1;

    let mut anmf = Vec::new();
    anmf.extend_from_slice(&u24(0)); // x / 2
    anmf.extend_from_slice(&u24(0)); // y / 2
    anmf.extend_from_slice(&u24(4 - 1)); // frame width - 1: smaller than the VP8's 7
    anmf.extend_from_slice(&u24(3 - 1)); // frame height - 1: smaller than the VP8's 5
    anmf.extend_from_slice(&u24(100)); // duration
    anmf.push(0); // blend, no dispose
    chunk(&mut anmf, b"ALPH", alph);
    chunk(&mut anmf, b"VP8 ", vp8);

    let mut vp8x = vec![0x12, 0, 0, 0]; // animation + alpha
    vp8x.extend_from_slice(&u24(7 - 1));
    vp8x.extend_from_slice(&u24(5 - 1));
    let mut body = b"WEBP".to_vec();
    chunk(&mut body, b"VP8X", &vp8x);
    chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0]);
    chunk(&mut body, b"ANMF", &anmf);
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);

    let mut a = Animation::new(&file[..]).expect("container parses");
    // Must fail cleanly, not panic.
    while let Some(f) = a.next_frame() {
        if f.is_err() {
            break;
        }
    }
}
