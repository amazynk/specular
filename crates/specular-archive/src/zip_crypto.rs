//! Encrypted ZIP entries: traditional PKWARE encryption ("ZipCrypto") and WinZip AES (AE-1 and
//! AE-2). Decryption turns an entry's stored bytes back into its compressed stream, so it works
//! with every compression method the `zip` module reads.
//!
//! **ZipCrypto** (APPNOTE 6.1): three 32-bit keys stirred by the password, a 12-byte header
//! whose last byte checks the password against the CRC's high byte (or, when the sizes follow
//! in a data descriptor, the high byte of the modification time). One wrong password in 256
//! passes the check; the decoder or the CRC then catches it, and either failure is reported as
//! a wrong password. The cipher is weak and the module only reads it.
//!
//! **WinZip AES** (method 99, extra field 0x9901): a salt (8, 12 or 16 bytes), a 2-byte
//! password verifier, the data in AES-CTR (little-endian counter from 1) and a 10-byte
//! HMAC-SHA1 of the encrypted data. Keys come from PBKDF2-HMAC-SHA1 with 1000 rounds. The
//! authentication code is checked before anything is decrypted, so tampered data never reaches
//! a decoder. AE-2 entries carry no CRC (it would leak facts about the plain text).

use crate::password::Password;
use crate::{Error, Result};
use aes::{Aes128, Aes192, Aes256};
use ctr::Ctr128LE;
use ctr::cipher::{KeyIvInit, StreamCipher};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use zeroize::Zeroizing;

/// The encryption of one entry, from its central-directory record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Crypt {
    /// Traditional PKWARE encryption; the header's last byte must be one of these.
    ZipCrypto { crc_byte: u8, time_byte: Option<u8> },
    /// WinZip AES; `strength` 1, 2, 3 is AES-128, -192, -256.
    Aes { strength: u8, ae2: bool },
    /// PKWARE strong encryption (flag bit 6) or an AES entry without a usable 0x9901 field.
    Unsupported,
}

/// The WinZip AES extra field (0x9901): vendor version, "AE", strength, actual method.
pub(crate) const AES_EXTRA: u16 = 0x9901;

/// How entry `flags`/`method` is encrypted, given its central-directory CRC, modification time
/// and 0x9901 extra field: the encryption and the method its decrypted bytes are compressed
/// with. `None` when the entry is not encrypted.
pub(crate) fn from_header(
    flags: u16,
    method: u16,
    crc: u32,
    time: u16,
    aes_extra: Option<&[u8]>,
) -> Option<(Crypt, u16)> {
    if method == 99 {
        let crypt = match aes_extra {
            Some(&[v0, v1, b'A', b'E', strength @ 1..=3, m0, m1, ..]) => {
                let version = u16::from_le_bytes([v0, v1]);
                let actual = u16::from_le_bytes([m0, m1]);
                if version == 1 || version == 2 {
                    return Some((
                        Crypt::Aes {
                            strength,
                            ae2: version == 2,
                        },
                        actual,
                    ));
                }
                Crypt::Unsupported
            }
            _ => Crypt::Unsupported,
        };
        return Some((crypt, method));
    }
    if flags & 1 == 0 {
        return None;
    }
    if flags & 0x40 != 0 {
        return Some((Crypt::Unsupported, method));
    }
    Some((
        Crypt::ZipCrypto {
            crc_byte: (crc >> 24) as u8,
            time_byte: (flags & 0x08 != 0).then_some((time >> 8) as u8),
        },
        method,
    ))
}

impl Crypt {
    /// Whether the entry's CRC is meaningful after decryption (AE-2 stores none).
    pub(crate) fn has_crc(self) -> bool {
        !matches!(self, Crypt::Aes { ae2: true, .. })
    }

    /// Bytes encryption adds around the compressed stream.
    pub(crate) fn overhead(self) -> usize {
        match self {
            Crypt::ZipCrypto { .. } => 12,
            Crypt::Aes { strength, .. } => salt_len(strength) + 2 + 10,
            Crypt::Unsupported => 0,
        }
    }

    /// What a failure to decode or check decrypted data most likely means. ZipCrypto's
    /// one-byte check lets one wrong password in 256 through, which then shows up as garbage;
    /// AES has a 16-bit verifier and an authentication code, so its errors stand.
    pub(crate) fn explain(self, e: Error) -> Error {
        match (self, e) {
            (Crypt::ZipCrypto { .. }, Error::Malformed(_) | Error::Checksum) => {
                Error::WrongPassword
            }
            (_, e) => e,
        }
    }
}

fn salt_len(strength: u8) -> usize {
    4 + 4 * strength as usize
}

/// The compressed stream of an entry stored as `data` (encryption header, encrypted bytes and
/// trailer), checked and decrypted with `password`.
pub(crate) fn decrypt(crypt: Crypt, data: &[u8], password: Option<&Password>) -> Result<Vec<u8>> {
    if crypt == Crypt::Unsupported {
        return Err(Error::Unsupported("ZIP encryption method".into()));
    }
    let Some(password) = password else {
        return Err(Error::Encrypted);
    };
    if data.len() < crypt.overhead() {
        return Err(Error::Malformed("ZIP: encrypted entry too short".into()));
    }
    match crypt {
        Crypt::ZipCrypto {
            crc_byte,
            time_byte,
        } => zipcrypto(data, password.bytes(), crc_byte, time_byte),
        Crypt::Aes { strength, .. } => winzip_aes(data, password.bytes(), strength),
        Crypt::Unsupported => Err(Error::Unsupported("ZIP encryption method".into())),
    }
}

/// The CRC-32 table step ZipCrypto's key schedule is built on.
fn crc_step(crc: u32, b: u8) -> u32 {
    const TABLE: [u32; 256] = {
        let mut t = [0u32; 256];
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
            t[i] = c;
            i += 1;
        }
        t
    };
    (crc >> 8) ^ TABLE[((crc ^ b as u32) & 0xFF) as usize]
}

/// ZipCrypto's three keys (wiped when dropped: they are as good as the password).
struct Keys([u32; 3]);

impl Drop for Keys {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

impl Keys {
    fn new(password: &[u8]) -> Keys {
        let mut k = Keys([0x1234_5678, 0x2345_6789, 0x3456_7890]);
        for &b in password {
            k.update(b);
        }
        k
    }

    fn update(&mut self, b: u8) {
        let k = &mut self.0;
        k[0] = crc_step(k[0], b);
        k[1] = k[1]
            .wrapping_add(k[0] & 0xFF)
            .wrapping_mul(134_775_813)
            .wrapping_add(1);
        k[2] = crc_step(k[2], (k[1] >> 24) as u8);
    }

    fn decrypt(&mut self, c: u8) -> u8 {
        let t = (self.0[2] | 2) & 0xFFFF;
        let p = c ^ (t.wrapping_mul(t ^ 1) >> 8) as u8;
        self.update(p);
        p
    }
}

fn zipcrypto(data: &[u8], password: &[u8], crc_byte: u8, time_byte: Option<u8>) -> Result<Vec<u8>> {
    let mut keys = Keys::new(password);
    let (header, body) = data.split_at(12);
    let mut check = 0;
    for &c in header {
        check = keys.decrypt(c);
    }
    // Writers that put the sizes in a data descriptor check against the time; some still use
    // the CRC, which the central directory always has.
    if check != crc_byte && Some(check) != time_byte {
        return Err(Error::WrongPassword);
    }
    Ok(body.iter().map(|&c| keys.decrypt(c)).collect())
}

fn winzip_aes(data: &[u8], password: &[u8], strength: u8) -> Result<Vec<u8>> {
    let salt_len = salt_len(strength);
    let key_len = 8 + 8 * strength as usize;
    let (salt, rest) = data.split_at(salt_len);
    let (verifier, rest) = rest.split_at(2);
    let (body, code) = rest.split_at(rest.len() - 10);
    // Encryption key, authentication key, password verifier.
    let mut keys = Zeroizing::new([0u8; 2 * 32 + 2]);
    let keys = &mut keys[..2 * key_len + 2];
    pbkdf2::pbkdf2_hmac::<Sha1>(password, salt, 1000, keys);
    let (enc, rest) = keys.split_at(key_len);
    let (auth, check) = rest.split_at(key_len);
    // Not secret (it is stored next to the data), so a plain comparison is fine.
    if check != verifier {
        return Err(Error::WrongPassword);
    }
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(auth)
        .map_err(|_| Error::Malformed("ZIP: AES key".into()))?;
    mac.update(body);
    mac.verify_truncated_left(code).map_err(|_| {
        Error::Malformed("ZIP: AES authentication code mismatch (damaged or altered entry)".into())
    })?;
    let mut out = body.to_vec();
    let mut iv = [0u8; 16];
    iv[0] = 1;
    let bad_key = |_| Error::Malformed("ZIP: AES key".into());
    match strength {
        1 => Ctr128LE::<Aes128>::new_from_slices(enc, &iv)
            .map_err(bad_key)?
            .apply_keystream(&mut out),
        2 => Ctr128LE::<Aes192>::new_from_slices(enc, &iv)
            .map_err(bad_key)?
            .apply_keystream(&mut out),
        _ => Ctr128LE::<Aes256>::new_from_slices(enc, &iv)
            .map_err(bad_key)?
            .apply_keystream(&mut out),
    }
    Ok(out)
}

/// Writers for tests: ZipCrypto and WinZip AES archives built in memory, so the readers are
/// checked against archives made independently of them (and against `7z`-made fixtures).
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// `plain` (an entry's compressed stream) encrypted with ZipCrypto; `check` is the header's
    /// last byte. `seed` stands in for the random header bytes.
    pub(crate) fn zipcrypto_encrypt(plain: &[u8], password: &[u8], check: u8, seed: u8) -> Vec<u8> {
        let mut keys = Keys::new(password);
        let mut header: Vec<u8> = (0..11)
            .map(|i| seed.wrapping_mul(31).wrapping_add(i))
            .collect();
        header.push(check);
        header
            .iter()
            .chain(plain)
            .map(|&p| {
                let t = (keys.0[2] | 2) & 0xFFFF;
                let c = p ^ (t.wrapping_mul(t ^ 1) >> 8) as u8;
                keys.update(p);
                c
            })
            .collect()
    }

    /// `plain` encrypted as WinZip AES with `strength` (1–3) and the salt `salt`.
    pub(crate) fn aes_encrypt(plain: &[u8], password: &[u8], strength: u8, salt: &[u8]) -> Vec<u8> {
        let key_len = 8 + 8 * strength as usize;
        assert_eq!(salt.len(), salt_len(strength));
        let mut keys = vec![0u8; 2 * key_len + 2];
        pbkdf2::pbkdf2_hmac::<Sha1>(password, salt, 1000, &mut keys);
        let mut body = plain.to_vec();
        let mut iv = [0u8; 16];
        iv[0] = 1;
        let enc = &keys[..key_len];
        match strength {
            1 => Ctr128LE::<Aes128>::new_from_slices(enc, &iv)
                .unwrap()
                .apply_keystream(&mut body),
            2 => Ctr128LE::<Aes192>::new_from_slices(enc, &iv)
                .unwrap()
                .apply_keystream(&mut body),
            _ => Ctr128LE::<Aes256>::new_from_slices(enc, &iv)
                .unwrap()
                .apply_keystream(&mut body),
        }
        let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(&keys[key_len..2 * key_len]).unwrap();
        mac.update(&body);
        let code = mac.finalize().into_bytes();
        let mut out = salt.to_vec();
        out.extend_from_slice(&keys[2 * key_len..]);
        out.extend_from_slice(&body);
        out.extend_from_slice(&code[..10]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    fn pw(s: &str) -> Password {
        Password::new(s.into())
    }

    #[test]
    fn pbkdf2_sha1_matches_rfc_6070() {
        let mut out = [0u8; 20];
        pbkdf2::pbkdf2_hmac::<Sha1>(b"password", b"salt", 2, &mut out);
        assert_eq!(
            out,
            [
                0xea, 0x6c, 0x01, 0x4d, 0xc7, 0x2d, 0x6f, 0x8c, 0xcd, 0x1e, 0xd9, 0x2a, 0xce, 0x1d,
                0x41, 0xf0, 0xd8, 0xde, 0x89, 0x57
            ]
        );
    }

    #[test]
    fn zipcrypto_round_trip_and_wrong_password() {
        let plain = b"compressed stream bytes".to_vec();
        let c = Crypt::ZipCrypto {
            crc_byte: 0xAB,
            time_byte: None,
        };
        let data = zipcrypto_encrypt(&plain, b"secret", 0xAB, 7);
        assert_eq!(decrypt(c, &data, Some(&pw("secret"))).unwrap(), plain);
        // A wrong password almost always fails the check byte; this one does.
        assert!(matches!(
            decrypt(c, &data, Some(&pw("Secret"))),
            Err(Error::WrongPassword)
        ));
        assert!(matches!(decrypt(c, &data, None), Err(Error::Encrypted)));
        // Time byte accepted when the sizes are in a data descriptor.
        let t = Crypt::ZipCrypto {
            crc_byte: 0x00,
            time_byte: Some(0x5C),
        };
        let data = zipcrypto_encrypt(&plain, b"secret", 0x5C, 9);
        assert_eq!(decrypt(t, &data, Some(&pw("secret"))).unwrap(), plain);
    }

    #[test]
    fn aes_all_strengths_round_trip() {
        let plain: Vec<u8> = (0..1000u32).map(|i| (i * 7 + i / 13) as u8).collect();
        for strength in 1..=3u8 {
            let salt: Vec<u8> = (0..salt_len(strength) as u8).collect();
            let data = aes_encrypt(&plain, "pässword".as_bytes(), strength, &salt);
            let c = Crypt::Aes {
                strength,
                ae2: true,
            };
            assert_eq!(decrypt(c, &data, Some(&pw("pässword"))).unwrap(), plain);
            assert!(matches!(
                decrypt(c, &data, Some(&pw("password"))),
                Err(Error::WrongPassword)
            ));
            assert!(matches!(
                decrypt(c, &data, Some(&pw(""))),
                Err(Error::WrongPassword)
            ));
        }
    }

    #[test]
    fn tampered_aes_entry_is_refused() {
        let plain = vec![0x42u8; 300];
        let salt = [9u8; 16];
        let c = Crypt::Aes {
            strength: 3,
            ae2: false,
        };
        let good = aes_encrypt(&plain, b"pw", 3, &salt);
        // The authentication code, then a data byte.
        for at in [good.len() - 1, 16 + 2 + 100] {
            let mut bad = good.clone();
            bad[at] ^= 1;
            assert!(
                matches!(decrypt(c, &bad, Some(&pw("pw"))), Err(Error::Malformed(_))),
                "byte {at}"
            );
        }
    }

    #[test]
    fn truncated_data_never_panics() {
        let plain = vec![1u8; 64];
        let aes = aes_encrypt(&plain, b"pw", 1, &[3u8; 8]);
        let zc = zipcrypto_encrypt(&plain, b"pw", 0x11, 1);
        for n in 0..aes.len() {
            let c = Crypt::Aes {
                strength: 1,
                ae2: false,
            };
            assert!(decrypt(c, &aes[..n], Some(&pw("pw"))).is_err());
        }
        for n in 0..zc.len() {
            let c = Crypt::ZipCrypto {
                crc_byte: 0x11,
                time_byte: None,
            };
            let _ = decrypt(c, &zc[..n], Some(&pw("pw")));
        }
    }

    #[test]
    fn header_fields() {
        assert_eq!(from_header(0, 8, 0, 0, None), None);
        assert_eq!(
            from_header(1, 8, 0xAB00_0000, 0x1234, None),
            Some((
                Crypt::ZipCrypto {
                    crc_byte: 0xAB,
                    time_byte: None
                },
                8
            ))
        );
        assert_eq!(
            from_header(9, 0, 0, 0x5C00, None).map(|c| c.0),
            Some(Crypt::ZipCrypto {
                crc_byte: 0,
                time_byte: Some(0x5C)
            })
        );
        assert_eq!(
            from_header(1, 99, 0, 0, Some(&[2, 0, b'A', b'E', 3, 8, 0])),
            Some((
                Crypt::Aes {
                    strength: 3,
                    ae2: true
                },
                8
            ))
        );
        assert_eq!(
            from_header(1, 99, 0, 0, Some(&[2, 0, b'A', b'E', 4, 8, 0])).map(|c| c.0),
            Some(Crypt::Unsupported)
        );
        assert_eq!(
            from_header(0x41, 8, 0, 0, None).map(|c| c.0),
            Some(Crypt::Unsupported)
        );
    }
}
