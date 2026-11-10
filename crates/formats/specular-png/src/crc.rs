//! CRC-32 (ISO-HDLC, as used by PNG chunks), slicing-by-8.

const fn tables() -> [[u32; 256]; 8] {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[0][i] = c;
        i += 1;
    }
    let mut i = 0;
    while i < 256 {
        let mut s = 1;
        while s < 8 {
            let p = t[s - 1][i];
            t[s][i] = (p >> 8) ^ t[0][(p & 0xff) as usize];
            s += 1;
        }
        i += 1;
    }
    t
}

static T: [[u32; 256]; 8] = tables();

/// CRC-32 of `data`.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    let mut chunks = data.chunks_exact(8);
    for b in &mut chunks {
        let lo = c ^ u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        c = T[7][(lo & 0xff) as usize]
            ^ T[6][((lo >> 8) & 0xff) as usize]
            ^ T[5][((lo >> 16) & 0xff) as usize]
            ^ T[4][(lo >> 24) as usize]
            ^ T[3][b[4] as usize]
            ^ T[2][b[5] as usize]
            ^ T[1][b[6] as usize]
            ^ T[0][b[7] as usize];
    }
    for &b in chunks.remainder() {
        c = T[0][((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

#[cfg(test)]
mod tests {
    #[test]
    fn known() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(super::crc32(b"IEND"), 0xAE42_6082);
    }
}
