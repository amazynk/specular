//! AES-128-CBC with PKCS#7 padding, the encryption of HLS `METHOD=AES-128` segments (RFC 8216
//! §4.3.2.4). The block cipher is RustCrypto's `aes`; the chaining is the few lines here.

use aes::Aes128;
use aes::cipher::{Array, BlockCipherDecrypt, KeyInit};

/// Why a segment did not decrypt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The key is not 16 bytes.
    KeyLength,
    /// The data is not whole blocks.
    Length,
    /// The padding is wrong (a wrong key or IV).
    Padding,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::KeyLength => "the key is not 16 bytes",
            Error::Length => "the encrypted segment is cut short",
            Error::Padding => "the segment did not decrypt (wrong key)",
        })
    }
}

/// Decrypts `data` in place and returns the plain length.
pub fn decrypt(data: &mut [u8], key: &[u8], iv: &[u8; 16]) -> Result<usize, Error> {
    let key: [u8; 16] = key.try_into().map_err(|_| Error::KeyLength)?;
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err(Error::Length);
    }
    let cipher = Aes128::new(&Array::from(key));
    let mut prev = *iv;
    for chunk in data.chunks_exact_mut(16) {
        let mut this = [0u8; 16];
        this.copy_from_slice(chunk);
        let mut b = Array::from(this);
        cipher.decrypt_block(&mut b);
        for (o, (p, c)) in chunk.iter_mut().zip(b.iter().zip(prev.iter())) {
            *o = p ^ c;
        }
        prev = this;
    }
    let pad = usize::from(data[data.len() - 1]);
    if pad == 0
        || pad > 16
        || data[data.len() - pad..]
            .iter()
            .any(|&b| usize::from(b) != pad)
    {
        return Err(Error::Padding);
    }
    Ok(data.len() - pad)
}

/// Encrypts (tests and fixtures only: the app never encrypts).
pub fn encrypt(plain: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> Vec<u8> {
    use aes::cipher::BlockCipherEncrypt;
    let cipher = Aes128::new(&Array::from(*key));
    let pad = 16 - plain.len() % 16;
    let mut data = plain.to_vec();
    data.extend(std::iter::repeat_n(pad as u8, pad));
    let mut prev = *iv;
    for chunk in data.chunks_exact_mut(16) {
        let mut b = [0u8; 16];
        for (o, (p, c)) in b.iter_mut().zip(chunk.iter().zip(prev.iter())) {
            *o = p ^ c;
        }
        let mut a = Array::from(b);
        cipher.encrypt_block(&mut a);
        chunk.copy_from_slice(&a);
        prev = a.into();
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NIST SP 800-38A F.2.2 (CBC-AES128.Decrypt), first two blocks, plus PKCS#7 round trips.
    #[test]
    fn decrypts_the_nist_vectors() {
        let key = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv: [u8; 16] = std::array::from_fn(|i| i as u8);
        let plain = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a, 0xae, 0x2d, 0x8a, 0x57, 0x1e, 0x03, 0xac, 0x9c, 0x9e, 0xb7, 0x6f, 0xac,
            0x45, 0xaf, 0x8e, 0x51,
        ];
        let cipher = [
            0x76, 0x49, 0xab, 0xac, 0x81, 0x19, 0xb2, 0x46, 0xce, 0xe9, 0x8e, 0x9b, 0x12, 0xe9,
            0x19, 0x7d, 0x50, 0x86, 0xcb, 0x9b, 0x50, 0x72, 0x19, 0xee, 0x95, 0xdb, 0x11, 0x3a,
            0x91, 0x76, 0x78, 0xb2,
        ];
        let enc = encrypt(&plain, &key, &iv);
        assert_eq!(enc[..32], cipher);
        let mut d = enc.clone();
        let n = decrypt(&mut d, &key, &iv).unwrap();
        assert_eq!(&d[..n], &plain);
        for len in [0usize, 1, 15, 16, 17, 1000] {
            let p: Vec<u8> = (0..len).map(|i| (i * 31) as u8).collect();
            let mut e = encrypt(&p, &key, &iv);
            let n = decrypt(&mut e, &key, &iv).unwrap();
            assert_eq!(&e[..n], &p[..]);
        }
        let mut e = encrypt(b"hello", &key, &iv);
        assert_eq!(decrypt(&mut e, &[0; 16], &iv), Err(Error::Padding));
        assert_eq!(decrypt(&mut [0; 15], &key, &iv), Err(Error::Length));
        assert_eq!(decrypt(&mut [0; 16], &key[..5], &iv), Err(Error::KeyLength));
    }
}
