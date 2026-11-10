//! GSM 06.10 against libgsm's decoding of the same streams (made by `data/gen.sh`).

fn pcm(name: &str) -> Vec<i16> {
    let d = std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    d.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn plain_matches_libgsm() {
    let d = read("sweep.gsm");
    assert!(specular_gsm::is_plain_stream(&d));
    let ours = specular_gsm::decode_plain(&d);
    let want = pcm("sweep.libgsm.pcm");
    assert_eq!(ours.len(), want.len());
    let diff = ours.iter().zip(&want).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "{diff} samples differ");
}

#[test]
fn microsoft_blocks_match_libgsm() {
    let d = read("sweep-ms.wav");
    // The data chunk after the 20-byte format and the fact chunk.
    let at = d.windows(4).position(|w| w == b"data").unwrap();
    let len = u32::from_le_bytes(d[at + 4..at + 8].try_into().unwrap()) as usize;
    let body = &d[at + 8..(at + 8 + len).min(d.len())];
    let ours = specular_gsm::decode_ms(body);
    let want = pcm("sweep-ms.libgsm.pcm");
    assert_eq!(ours.len(), want.len());
    let diff = ours.iter().zip(&want).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "{diff} samples differ");
}
