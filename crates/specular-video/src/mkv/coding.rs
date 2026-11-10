//! A Matroska track's `ContentEncodings` (RFC 9559 5.1.4.1.31): how its frames and codec
//! private data are stored, and undoing that for the demuxer ([`crate::mkv`]), the frame
//! reader ([`crate::mp4::Layout::Mkv`]) and the subtitle reader ([`crate::extras`]).
//!
//! - Header stripping (`ContentCompAlgo` 3): `ContentCompSettings` goes back in front of the
//!   data. mkvmerge's default for MP3, AC-3, DTS, H.264, HEVC and more.
//! - zlib (algo 0): inflated with the in-tree inflater; bzip2 (algo 1): the in-tree bzip2.
//!   Both are capped at [`MAX_FRAME`] bytes and reserve their output from the memory budget.
//! - LZO (algo 2), other algos and encryption can't be undone: [`Coding::refusal`] names why.
//!
//! The encodings are undone from the highest `ContentEncodingOrder` down. Each applies where
//! its `ContentEncodingScope` says: 1 the frames (each frame of a lace on its own), 2 the
//! codec private data, 4 the next encoding's own data (its `ContentCompSettings`; the next is
//! the one undone after it).

use super::{El, children, uint};
use std::borrow::Cow;

pub(crate) const CONTENT_ENCODING: u32 = 0x6240;
pub(crate) const CONTENT_ENCODING_ORDER: u32 = 0x5031;
pub(crate) const CONTENT_ENCODING_SCOPE: u32 = 0x5032;
pub(crate) const CONTENT_ENCODING_TYPE: u32 = 0x5033;
pub(crate) const CONTENT_COMPRESSION: u32 = 0x5034;
pub(crate) const CONTENT_ENCRYPTION: u32 = 0x5035;
pub(crate) const CONTENT_COMP_ALGO: u32 = 0x4254;
pub(crate) const CONTENT_COMP_SETTINGS: u32 = 0x4255;

const SCOPE_FRAMES: u64 = 1;
const SCOPE_PRIVATE: u64 = 2;
const SCOPE_NEXT: u64 = 4;

/// Largest frame a compressed frame may expand to (an 8K 16-bit RGBA picture fits); the
/// memory budget is asked first, so a file can't make one balloon past free memory.
pub(crate) const MAX_FRAME: usize = 320 << 20;
/// Largest codec private data a compressed one may expand to.
const MAX_PRIVATE: usize = 16 << 20;

/// One way the data was stored.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// These bytes were cut from the front of each frame.
    Strip(Vec<u8>),
    Zlib,
    Bzip2,
    Lzo,
    /// Another compression algorithm.
    Other,
    Encrypted,
}

/// Why a track's frames can't be read: the codec code its track gets (see
/// [`crate::mp4::Codec::name`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Encrypted,
    Lzo,
    Compressed,
}

impl Refusal {
    pub(crate) fn code(self) -> [u8; 4] {
        match self {
            Refusal::Encrypted => *b"encr",
            Refusal::Lzo => *b"cLZO",
            Refusal::Compressed => *b"cmpr",
        }
    }
}

/// A track's encodings, in the order they are undone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coding {
    frames: Vec<Step>,
    private: Vec<Step>,
}

impl Coding {
    /// The encodings in the `ContentEncodings` element `e`.
    pub(crate) fn read(data: &[u8], e: &El) -> Coding {
        // (order, scope, step)
        let mut all: Vec<(u64, u64, Step)> = Vec::new();
        for enc in children(data, e.body, e.end).filter(|x| x.id == CONTENT_ENCODING) {
            let (mut order, mut scope, mut kind) = (0, SCOPE_FRAMES, 0);
            let (mut algo, mut settings, mut encrypted) = (0, Vec::new(), false);
            for k in children(data, enc.body, enc.end) {
                match k.id {
                    CONTENT_ENCODING_ORDER => order = uint(data, &k).unwrap_or(0),
                    CONTENT_ENCODING_SCOPE => scope = uint(data, &k).unwrap_or(SCOPE_FRAMES),
                    CONTENT_ENCODING_TYPE => kind = uint(data, &k).unwrap_or(0),
                    CONTENT_ENCRYPTION => encrypted = true,
                    CONTENT_COMPRESSION => {
                        for s in children(data, k.body, k.end) {
                            match s.id {
                                CONTENT_COMP_ALGO => algo = uint(data, &s).unwrap_or(0),
                                CONTENT_COMP_SETTINGS => {
                                    settings = data.get(s.body..s.end).unwrap_or_default().to_vec();
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            let step = if encrypted || kind == 1 {
                Step::Encrypted
            } else if kind != 0 {
                Step::Other
            } else {
                match algo {
                    0 => Step::Zlib,
                    1 => Step::Bzip2,
                    2 => Step::Lzo,
                    3 => Step::Strip(settings),
                    _ => Step::Other,
                }
            };
            all.push((order, scope, step));
        }
        // The highest order is undone first.
        all.sort_by_key(|x| std::cmp::Reverse(x.0));
        // Scope 4: the encoding stored the next one's settings encoded.
        for i in (0..all.len().saturating_sub(1)).rev() {
            if all[i].1 & SCOPE_NEXT == 0 {
                continue;
            }
            let outer = all[i].2.clone();
            if let Step::Strip(s) = &all[i + 1].2 {
                all[i + 1].2 = match undo(&outer, s, MAX_PRIVATE) {
                    Some(s) => Step::Strip(s.into_owned()),
                    None => Step::Other,
                };
            }
        }
        let pick = |bit: u64| -> Vec<Step> {
            all.iter()
                .filter(|(_, s, _)| s & bit != 0)
                .map(|(_, _, st)| st.clone())
                .collect()
        };
        Coding {
            frames: pick(SCOPE_FRAMES),
            private: pick(SCOPE_PRIVATE),
        }
    }

    /// Frames are stored as they are read.
    pub fn is_plain(&self) -> bool {
        self.frames.is_empty()
    }

    /// Why the frames or the codec private data can't be read, if they can't.
    pub(crate) fn refusal(&self) -> Option<Refusal> {
        let steps = || self.frames.iter().chain(&self.private);
        if steps().any(|s| *s == Step::Encrypted) {
            Some(Refusal::Encrypted)
        } else if steps().any(|s| *s == Step::Lzo) {
            Some(Refusal::Lzo)
        } else if steps().any(|s| *s == Step::Other) {
            Some(Refusal::Compressed)
        } else {
            None
        }
    }

    /// A frame's bytes as stored (one frame of a lace), made readable; `None` when they
    /// can't be (damaged, too large for [`MAX_FRAME`] or the memory budget, or refused).
    pub fn frame<'a>(&self, raw: &'a [u8]) -> Option<Cow<'a, [u8]>> {
        undo_all(&self.frames, raw, MAX_FRAME)
    }

    /// The codec private data as stored, made readable.
    pub(crate) fn private<'a>(&self, raw: &'a [u8]) -> Option<Cow<'a, [u8]>> {
        undo_all(&self.private, raw, MAX_PRIVATE)
    }
}

fn undo_all<'a>(steps: &[Step], raw: &'a [u8], max: usize) -> Option<Cow<'a, [u8]>> {
    let mut cur = Cow::Borrowed(raw);
    for s in steps {
        cur = Cow::Owned(undo(s, &cur, max)?.into_owned());
    }
    Some(cur)
}

/// Undoes one step on `raw`, giving up beyond `max` bytes.
fn undo<'a>(s: &Step, raw: &'a [u8], max: usize) -> Option<Cow<'a, [u8]>> {
    match s {
        Step::Strip(h) => {
            if h.is_empty() {
                return Some(Cow::Borrowed(raw));
            }
            let n = h.len().checked_add(raw.len()).filter(|&n| n <= max)?;
            let mut v = Vec::with_capacity(n);
            v.extend_from_slice(h);
            v.extend_from_slice(raw);
            Some(Cow::Owned(v))
        }
        Step::Zlib => specular_png::inflate_zlib(raw, max).ok().map(Cow::Owned),
        Step::Bzip2 => {
            // Asked for one byte more than allowed: a longer result means too large.
            let v = specular_xcf::bunzip2(raw, max as u64 + 1).ok()?;
            (v.len() <= max).then_some(Cow::Owned(v))
        }
        Step::Lzo | Step::Other | Step::Encrypted => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mkv::build::{el, uel};
    use crate::mkv::element;

    /// zlib-compressed `data` (zlib-rs, the test oracle).
    pub(crate) fn zlib(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
        let (c, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::new(6));
        assert_eq!(rc, zlib_rs::ReturnCode::Ok);
        c.to_vec()
    }

    fn encoding(order: u64, scope: u64, algo: u64, settings: &[u8]) -> Vec<u8> {
        let mut comp = uel(CONTENT_COMP_ALGO, algo);
        if !settings.is_empty() {
            comp.extend(el(CONTENT_COMP_SETTINGS, settings));
        }
        let mut b = uel(CONTENT_ENCODING_ORDER, order);
        b.extend(uel(CONTENT_ENCODING_SCOPE, scope));
        b.extend(el(CONTENT_COMPRESSION, &comp));
        el(CONTENT_ENCODING, &b)
    }

    fn coding(encs: &[Vec<u8>]) -> Coding {
        let f = el(0x6D80, &encs.concat());
        Coding::read(&f, &element(&f, 0).unwrap())
    }

    #[test]
    fn header_stripping_goes_back_in_front() {
        let c = coding(&[encoding(0, 1, 3, &[0xFF, 0xFB])]);
        assert_eq!(c.refusal(), None);
        assert!(!c.is_plain());
        assert_eq!(&*c.frame(&[0x90, 0x64]).unwrap(), &[0xFF, 0xFB, 0x90, 0x64]);
        // Scope 1 only: the codec private data is as stored.
        assert!(matches!(c.private(b"abc").unwrap(), Cow::Borrowed(b"abc")));
    }

    #[test]
    fn highest_order_is_undone_first() {
        // Stored: strip "AB" (order 0), then zlib over the rest (order 1).
        let z = zlib(b"CDEF");
        let c = coding(&[encoding(0, 1, 3, b"AB"), encoding(1, 1, 0, &[])]);
        assert_eq!(&*c.frame(&z).unwrap(), b"ABCDEF");
        // Listed the other way round, the same.
        let c = coding(&[encoding(1, 1, 0, &[]), encoding(0, 1, 3, b"AB")]);
        assert_eq!(&*c.frame(&z).unwrap(), b"ABCDEF");
    }

    #[test]
    fn scope_two_is_the_codec_private_only() {
        let z = zlib(b"private");
        let c = coding(&[encoding(0, 2, 0, &[])]);
        assert!(c.is_plain());
        assert_eq!(&*c.private(&z).unwrap(), b"private");
        assert!(matches!(c.frame(b"x").unwrap(), Cow::Borrowed(b"x")));
    }

    #[test]
    fn scope_four_encodes_the_next_settings() {
        let z = zlib(b"HEAD");
        let c = coding(&[encoding(1, 4, 0, &[]), encoding(0, 1, 3, &z)]);
        assert_eq!(&*c.frame(b"tail").unwrap(), b"HEADtail");
    }

    #[test]
    fn bzip2_frames() {
        // "hello bzip2 frame" as bzip2 (Python's bz2, level 9).
        const BZ: &[u8] = &[
            66, 90, 104, 57, 49, 65, 89, 38, 83, 89, 178, 204, 155, 178, 0, 0, 4, 25, 128, 64, 0,
            16, 0, 51, 102, 208, 16, 32, 0, 34, 153, 25, 15, 83, 122, 132, 0, 0, 79, 250, 221, 39,
            240, 194, 96, 44, 93, 201, 20, 225, 66, 66, 203, 50, 110, 200,
        ];
        let c = coding(&[encoding(0, 1, 1, &[])]);
        assert_eq!(c.refusal(), None);
        assert_eq!(&*c.frame(BZ).unwrap(), b"hello bzip2 frame");
        assert!(undo(&Step::Bzip2, BZ, 16).is_none());
        assert!(c.frame(&BZ[..20]).is_none());
    }

    #[test]
    fn lzo_encryption_and_unknown_are_refused() {
        assert_eq!(
            coding(&[encoding(0, 1, 2, &[])]).refusal(),
            Some(Refusal::Lzo)
        );
        assert_eq!(
            coding(&[encoding(0, 1, 9, &[])]).refusal(),
            Some(Refusal::Compressed)
        );
        let enc = el(CONTENT_ENCODING, &el(CONTENT_ENCRYPTION, &[]));
        let c = coding(&[enc]);
        assert_eq!(c.refusal(), Some(Refusal::Encrypted));
        assert!(c.frame(b"x").is_none());
        assert_eq!(Coding::default().refusal(), None);
    }

    #[test]
    fn a_zlib_bomb_stops_at_the_cap() {
        let z = zlib(&vec![0u8; 4 << 20]);
        assert!(undo(&Step::Zlib, &z, 1 << 20).is_none());
        assert_eq!(undo(&Step::Zlib, &z, 4 << 20).unwrap().len(), 4 << 20);
        // Damaged data is no frame.
        assert!(undo(&Step::Zlib, &z[..z.len() / 2], 4 << 20).is_none());
    }
}
