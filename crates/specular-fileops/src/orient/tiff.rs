//! Reading and writing the Orientation tag (0x0112) of a TIFF structure: a whole TIFF file
//! or the Exif block inside a JPEG, PNG, WebP, HEIF or JPEG XL.
//!
//! An existing tag is patched where it stands (two bytes). Without one, IFD0 is copied to the
//! end with the tag added in order, and the header points at the copy: every other offset in
//! the file stays valid because nothing before the end moves.

/// Byte order and flavour of a TIFF structure.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tiff {
    pub le: bool,
    pub big: bool,
}

pub(crate) const ORIENTATION: u16 = 0x0112;
const EXIF_IFD: u16 = 0x8769;
const PIXEL_X: u16 = 0xA002;
const PIXEL_Y: u16 = 0xA003;

impl Tiff {
    pub fn detect(b: &[u8]) -> Option<Tiff> {
        let le = match b.get(..2)? {
            b"II" => true,
            b"MM" => false,
            _ => return None,
        };
        let t = Tiff { le, big: false };
        match t.u16(b, 2)? {
            42 => Some(t),
            43 if t.u16(b, 4)? == 8 => Some(Tiff { le, big: true }),
            _ => None,
        }
    }

    pub fn u16(&self, b: &[u8], o: usize) -> Option<u16> {
        let x = b.get(o..o + 2)?;
        Some(if self.le {
            u16::from_le_bytes([x[0], x[1]])
        } else {
            u16::from_be_bytes([x[0], x[1]])
        })
    }

    pub fn u32(&self, b: &[u8], o: usize) -> Option<u32> {
        let x: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(x)
        } else {
            u32::from_be_bytes(x)
        })
    }

    pub fn u64(&self, b: &[u8], o: usize) -> Option<u64> {
        let x: [u8; 8] = b.get(o..o + 8)?.try_into().ok()?;
        Some(if self.le {
            u64::from_le_bytes(x)
        } else {
            u64::from_be_bytes(x)
        })
    }

    fn put16(&self, v: u16) -> [u8; 2] {
        if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }

    fn put32(&self, v: u32) -> [u8; 4] {
        if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }

    fn put64(&self, v: u64) -> [u8; 8] {
        if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }

    /// Offset of IFD0.
    pub fn ifd0(&self, b: &[u8]) -> Option<usize> {
        if self.big {
            usize::try_from(self.u64(b, 8)?).ok()
        } else {
            Some(self.u32(b, 4)? as usize)
        }
    }

    fn entry_size(&self) -> usize {
        if self.big { 20 } else { 12 }
    }

    fn count_size(&self) -> usize {
        if self.big { 8 } else { 2 }
    }

    /// Number of entries of the IFD at `ifd`.
    fn count(&self, b: &[u8], ifd: usize) -> Option<usize> {
        if self.big {
            usize::try_from(self.u64(b, ifd)?).ok()
        } else {
            Some(self.u16(b, ifd)? as usize)
        }
    }

    /// Offsets of the entries of the IFD at `ifd`.
    pub fn entries(&self, b: &[u8], ifd: usize) -> Option<impl Iterator<Item = usize> + use<>> {
        let n = self.count(b, ifd)?;
        if n > 4096 {
            return None;
        }
        let first = ifd + self.count_size();
        let es = self.entry_size();
        b.get(first..first + n * es)?;
        Some((0..n).map(move |i| first + i * es))
    }

    /// Offset of the "next IFD" link of the IFD at `ifd`.
    pub fn next_link(&self, b: &[u8], ifd: usize) -> Option<usize> {
        let n = self.count(b, ifd)?;
        Some(ifd + self.count_size() + n * self.entry_size())
    }

    pub fn next_ifd(&self, b: &[u8], ifd: usize) -> Option<usize> {
        let l = self.next_link(b, ifd)?;
        let v = if self.big {
            usize::try_from(self.u64(b, l)?).ok()?
        } else {
            self.u32(b, l)? as usize
        };
        (v != 0).then_some(v)
    }

    /// The entry with `tag` in the IFD at `ifd`.
    pub fn find(&self, b: &[u8], ifd: usize, tag: u16) -> Option<usize> {
        self.entries(b, ifd)?.find(|&e| self.u16(b, e) == Some(tag))
    }

    /// Where the inline value of an entry starts.
    fn value_at(&self, e: usize) -> usize {
        if self.big { e + 12 } else { e + 8 }
    }

    /// A SHORT or LONG single value of an entry.
    pub fn scalar(&self, b: &[u8], e: usize) -> Option<u64> {
        let ty = self.u16(b, e + 2)?;
        let v = self.value_at(e);
        match ty {
            3 => self.u16(b, v).map(u64::from),
            4 => self.u32(b, v).map(u64::from),
            16 => self.u64(b, v),
            _ => None,
        }
    }

    /// Offset value of a pointer entry (sub-IFD, thumbnail).
    pub fn pointer(&self, b: &[u8], e: usize) -> Option<usize> {
        let ty = self.u16(b, e + 2)?;
        let v = self.value_at(e);
        match ty {
            4 | 13 => self.u32(b, v).map(|x| x as usize),
            16 | 18 => usize::try_from(self.u64(b, v)?).ok(),
            _ => None,
        }
    }

    /// Overwrite the single SHORT/LONG value of entry `e`. False when its type doesn't allow.
    pub fn set_scalar(&self, b: &mut [u8], e: usize, value: u64) -> bool {
        let Some(ty) = self.u16(b, e + 2) else {
            return false;
        };
        let v = self.value_at(e);
        let bytes: Vec<u8> = match ty {
            3 if value <= 0xFFFF => self.put16(value as u16).to_vec(),
            4 if value <= 0xFFFF_FFFF => self.put32(value as u32).to_vec(),
            16 => self.put64(value).to_vec(),
            _ => return false,
        };
        match b.get_mut(v..v + bytes.len()) {
            Some(d) => {
                d.copy_from_slice(&bytes);
                true
            }
            None => false,
        }
    }
}

/// The Orientation of IFD0 (1 when absent or out of range).
pub fn orientation(b: &[u8]) -> Option<u8> {
    let t = Tiff::detect(b)?;
    let ifd = t.ifd0(b)?;
    let e = t.find(b, ifd, ORIENTATION)?;
    let v = t.scalar(b, e)?;
    Some(if (1..=8).contains(&v) { v as u8 } else { 1 })
}

/// Set IFD0's Orientation to `o`, in place when the tag is there, else by appending a copy of
/// IFD0 with the tag. `None` when the structure doesn't parse.
pub fn set_orientation(b: &mut Vec<u8>, o: u8) -> Option<()> {
    let t = Tiff::detect(b)?;
    let ifd = t.ifd0(b)?;
    if let Some(e) = t.find(b, ifd, ORIENTATION)
        && t.set_scalar(b, e, u64::from(o))
    {
        return Some(());
    }
    add_entry(b, t, None, ORIENTATION, u64::from(o))
}

/// Append a copy of the IFD at `ifd` (IFD0 when `None`) with `tag` = SHORT `value` (an
/// existing `tag` entry is replaced), and point its parent link (the header for IFD0) at it.
fn add_entry(b: &mut Vec<u8>, t: Tiff, _ifd: Option<usize>, tag: u16, value: u64) -> Option<()> {
    let ifd = t.ifd0(b)?;
    let es = t.entry_size();
    let mut entries: Vec<Vec<u8>> = t
        .entries(b, ifd)?
        .filter(|&e| t.u16(b, e) != Some(tag))
        .map(|e| b[e..e + es].to_vec())
        .collect();
    let mut ne = Vec::with_capacity(es);
    ne.extend_from_slice(&t.put16(tag));
    ne.extend_from_slice(&t.put16(3));
    if t.big {
        ne.extend_from_slice(&t.put64(1));
        let mut v = [0u8; 8];
        v[..2].copy_from_slice(&t.put16(value as u16));
        ne.extend_from_slice(&v);
    } else {
        ne.extend_from_slice(&t.put32(1));
        let mut v = [0u8; 4];
        v[..2].copy_from_slice(&t.put16(value as u16));
        ne.extend_from_slice(&v);
    }
    let pos = entries
        .iter()
        .position(|e| t.u16(e, 0).is_some_and(|x| x > tag))
        .unwrap_or(entries.len());
    entries.insert(pos, ne);
    let next_link = t.next_link(b, ifd)?;
    let next = b
        .get(next_link..next_link + if t.big { 8 } else { 4 })?
        .to_vec();
    while !b.len().is_multiple_of(2) {
        b.push(0);
    }
    let at = b.len();
    if t.big {
        b.extend_from_slice(&t.put64(entries.len() as u64));
    } else {
        b.extend_from_slice(&t.put16(u16::try_from(entries.len()).ok()?));
    }
    for e in &entries {
        b.extend_from_slice(e);
    }
    b.extend_from_slice(&next);
    if t.big {
        let v = t.put64(at as u64);
        b[8..16].copy_from_slice(&v);
    } else {
        let v = t.put32(u32::try_from(at).ok()?);
        b[4..8].copy_from_slice(&v);
    }
    Some(())
}

/// After a lossless rotation that swapped width and height: swap the Exif
/// PixelXDimension/PixelYDimension values (left alone when they can't be swapped in place).
pub fn swap_pixel_dimensions(b: &mut [u8]) -> Option<()> {
    let t = Tiff::detect(b)?;
    let ifd0 = t.ifd0(b)?;
    let exif = t.pointer(b, t.find(b, ifd0, EXIF_IFD)?)?;
    let ex = t.find(b, exif, PIXEL_X)?;
    let ey = t.find(b, exif, PIXEL_Y)?;
    let (x, y) = (t.scalar(b, ex)?, t.scalar(b, ey)?);
    // Types may differ (SHORT and LONG); both must take the other's value.
    let bx = b.to_vec();
    let ok = t.set_scalar(b, ex, y) && t.set_scalar(b, ey, x);
    if !ok {
        b.copy_from_slice(&bx);
    }
    Some(())
}

/// The thumbnail (IFD1 JPEGInterchangeFormat) as (offset, length) within `b`.
pub fn thumbnail(b: &[u8]) -> Option<(usize, usize)> {
    let t = Tiff::detect(b)?;
    let ifd1 = t.next_ifd(b, t.ifd0(b)?)?;
    let off = t.scalar(b, t.find(b, ifd1, 0x0201)?)? as usize;
    let len = t.scalar(b, t.find(b, ifd1, 0x0202)?)? as usize;
    b.get(off..off.checked_add(len)?)?;
    Some((off, len))
}

/// Put a new thumbnail in the old one's place (it must not be longer) and record its length.
pub fn replace_thumbnail(b: &mut [u8], new: &[u8]) -> Option<()> {
    let (off, len) = thumbnail(b)?;
    if new.len() > len {
        return None;
    }
    let t = Tiff::detect(b)?;
    let ifd1 = t.next_ifd(b, t.ifd0(b)?)?;
    let le = t.find(b, ifd1, 0x0202)?;
    if !t.set_scalar(b, le, new.len() as u64) {
        return None;
    }
    b[off..off + new.len()].copy_from_slice(new);
    b[off + new.len()..off + len].fill(0);
    Some(())
}

/// Unlink IFD1 (the thumbnail) from IFD0: readers then find no thumbnail.
pub fn drop_thumbnail(b: &mut [u8]) -> Option<()> {
    let t = Tiff::detect(b)?;
    let l = t.next_link(b, t.ifd0(b)?)?;
    b.get_mut(l..l + if t.big { 8 } else { 4 })?.fill(0);
    Some(())
}

/// The smallest Exif TIFF holding only an Orientation: big-endian, one IFD0 entry.
pub fn minimal(o: u8) -> Vec<u8> {
    let mut v = b"MM\0\x2a\0\0\0\x08\0\x01".to_vec();
    v.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, o, 0, 0]);
    v.extend_from_slice(&[0, 0, 0, 0]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_reads_back() {
        for o in 1..=8 {
            assert_eq!(orientation(&minimal(o)), Some(o));
        }
    }

    #[test]
    fn adds_and_patches() {
        // II, IFD0 with one entry (ImageWidth LONG 640), next 0.
        let mut b = b"II\x2a\0\x08\0\0\0".to_vec();
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&[0x00, 0x01, 4, 0, 1, 0, 0, 0]);
        b.extend_from_slice(&640u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(orientation(&b), None);
        set_orientation(&mut b, 6).unwrap();
        assert_eq!(orientation(&b), Some(6));
        let t = Tiff::detect(&b).unwrap();
        let ifd = t.ifd0(&b).unwrap();
        // ImageWidth still first and intact.
        let e = t.find(&b, ifd, 0x0100).unwrap();
        assert_eq!(t.scalar(&b, e), Some(640));
        let len = b.len();
        set_orientation(&mut b, 3).unwrap();
        assert_eq!(b.len(), len, "patched in place the second time");
        assert_eq!(orientation(&b), Some(3));
    }
}
