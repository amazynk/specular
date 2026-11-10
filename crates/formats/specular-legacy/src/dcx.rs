//! DCX: multi-page PCX (fax software, scanners). A little-endian magic 987654321
//! (`B1 68 DE 3A`), then up to 1023 little-endian `u32` page offsets ended by a zero; each page
//! is a whole PCX file running to the next page's offset (the end of the file for the last).

use crate::*;
use std::ops::Range;

pub const MAGIC: [u8; 4] = [0xB1, 0x68, 0xDE, 0x3A];
/// Most pages a DCX file lists.
pub const MAX_PAGES: usize = 1023;

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 8 && head[..4] == MAGIC
}

/// The byte range of each page's PCX file, in order. Offsets inside the table or past the end
/// of the file end the list (a cut-short file keeps the pages it has).
pub fn pages(data: &[u8]) -> Result<Vec<Range<usize>>> {
    if data.len() < 4 || data[..4] != MAGIC {
        return Err(Error::NotRecognised);
    }
    let mut starts = Vec::new();
    for i in 0..MAX_PAGES {
        let Some(o) = u32le(data, 4 + 4 * i) else {
            break;
        };
        if o == 0 {
            break;
        }
        let o = o as usize;
        // Inside the header or not after the previous page: corrupt from here on.
        if o < 8 + 4 * (i + 1) || o >= data.len() || starts.last().is_some_and(|&p| o <= p) {
            break;
        }
        starts.push(o);
    }
    if starts.is_empty() {
        return Err(Error::Truncated("no pages".into()));
    }
    let ends = starts.iter().skip(1).copied().chain([data.len()]);
    Ok(starts.iter().zip(ends).map(|(&s, e)| s..e).collect())
}

/// Header facts of `page`, with the page position in the coding ("page 2 of 5, …").
pub fn info(data: &[u8], page: usize) -> Result<Info> {
    let p = pages(data)?;
    let r = p.get(page).ok_or(Error::Malformed("no such page".into()))?;
    let mut i = pcx::info(&data[r.clone()])?;
    i.coding = if p.len() > 1 {
        format!("page {} of {}, {}", page + 1, p.len(), i.coding)
    } else {
        i.coding
    };
    Ok(i)
}

/// Decodes `page`.
pub fn decode(data: &[u8], page: usize) -> Result<Image> {
    let p = pages(data)?;
    let r = p.get(page).ok_or(Error::Malformed("no such page".into()))?;
    pcx::decode(&data[r.clone()])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An 8-bit greyscale raw (uncompressed) PCX filled with `v`.
    pub(crate) fn grey_pcx(w: u16, h: u16, v: u8) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[0] = 0x0A;
        d[1] = 5;
        d[2] = 0;
        d[3] = 8;
        d[8..10].copy_from_slice(&(w - 1).to_le_bytes());
        d[10..12].copy_from_slice(&(h - 1).to_le_bytes());
        d[65] = 1;
        d[66..68].copy_from_slice(&w.to_le_bytes());
        d.extend(std::iter::repeat_n(v, usize::from(w) * usize::from(h)));
        d
    }

    pub(crate) fn dcx(pages: &[Vec<u8>]) -> Vec<u8> {
        let mut d = MAGIC.to_vec();
        let mut at = 4 + 4 * (pages.len() + 1);
        for p in pages {
            d.extend_from_slice(&(at as u32).to_le_bytes());
            at += p.len();
        }
        d.extend_from_slice(&[0; 4]);
        for p in pages {
            d.extend_from_slice(p);
        }
        d
    }

    #[test]
    fn pages_decode() {
        let d = dcx(&[grey_pcx(5, 3, 10), grey_pcx(2, 7, 200)]);
        assert!(probe(&d));
        assert_eq!(pages(&d).unwrap().len(), 2);
        let i = info(&d, 1).unwrap();
        assert_eq!((i.width, i.height), (2, 7));
        assert_eq!(i.coding, "page 2 of 2, 8-bit grey");
        let a = decode(&d, 0).unwrap();
        assert_eq!((a.width, a.height), (5, 3));
        assert!(a.data.iter().all(|&v| v == 10));
        let b = decode(&d, 1).unwrap();
        assert!(b.data.iter().all(|&v| v == 200));
        assert!(decode(&d, 2).is_err());
    }

    #[test]
    fn malformed_files_fail_cleanly() {
        let d = dcx(&[grey_pcx(5, 3, 10), grey_pcx(2, 7, 200)]);
        for n in 0..d.len() {
            let t = &d[..n];
            if let Ok(p) = pages(t) {
                for k in 0..p.len() {
                    let _ = info(t, k);
                    let _ = decode(t, k);
                }
            }
        }
        // Offsets pointing backwards, into the table, or past the end.
        let mut bad = d.clone();
        bad[8..12].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(pages(&bad).unwrap().len(), 1);
        let mut bad = d.clone();
        bad[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(pages(&bad).is_err());
        // A table of 1023 entries with no terminator.
        let mut long = MAGIC.to_vec();
        for i in 0..2000u32 {
            long.extend_from_slice(&(5000 + i).to_le_bytes());
        }
        assert!(pages(&long).unwrap().len() <= MAX_PAGES);
    }
}
