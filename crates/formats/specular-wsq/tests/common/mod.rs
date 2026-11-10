#![allow(dead_code)]

/// A binary PGM (P5, maxval 255): width, height, pixels.
pub fn pgm(d: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while d[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !d[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(
            std::str::from_utf8(&d[s..i])
                .unwrap()
                .parse::<u32>()
                .unwrap(),
        );
    }
    assert_eq!(&d[..2], b"P5");
    assert_eq!(fields[2], 255);
    (fields[0], fields[1], d[i + 1..].to_vec())
}

pub fn fnv(px: &[u8]) -> u64 {
    px.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// (differing pixels, largest difference).
pub fn diff(a: &[u8], b: &[u8]) -> (usize, u8) {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x != y)
        .fold((0, 0), |(n, m), (x, y)| (n + 1, m.max(x.abs_diff(*y))))
}
