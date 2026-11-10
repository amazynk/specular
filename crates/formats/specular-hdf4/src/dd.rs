//! The data descriptor table: where every (tag, reference) element lies.

use std::collections::{HashMap, HashSet};

use crate::{Error, Result, be16, be32, limits};

pub(crate) const TAG_NULL: u16 = 1;
pub(crate) const TAG_LINKED: u16 = 20;
pub(crate) const TAG_VERSION: u16 = 30;
pub(crate) const TAG_COMPRESSED: u16 = 40;
pub(crate) const TAG_CHUNK: u16 = 61;
pub(crate) const TAG_NT: u16 = 106;
pub(crate) const TAG_ID8: u16 = 200;
pub(crate) const TAG_IP8: u16 = 201;
pub(crate) const TAG_RI8: u16 = 202;
pub(crate) const TAG_CI8: u16 = 203;
pub(crate) const TAG_II8: u16 = 204;
pub(crate) const TAG_ID: u16 = 300;
pub(crate) const TAG_LUT: u16 = 301;
pub(crate) const TAG_RI: u16 = 302;
pub(crate) const TAG_CI: u16 = 303;
pub(crate) const TAG_RIG: u16 = 306;
pub(crate) const TAG_SDG: u16 = 700;
pub(crate) const TAG_SDD: u16 = 701;
pub(crate) const TAG_SD: u16 = 702;
pub(crate) const TAG_SDL: u16 = 704;
pub(crate) const TAG_SDU: u16 = 705;
pub(crate) const TAG_SDF: u16 = 706;
pub(crate) const TAG_SDM: u16 = 707;
pub(crate) const TAG_SDC: u16 = 708;
pub(crate) const TAG_CAL: u16 = 731;
pub(crate) const TAG_FV: u16 = 732;
pub(crate) const TAG_NDG: u16 = 720;
pub(crate) const TAG_VH: u16 = 1962;
pub(crate) const TAG_VS: u16 = 1963;
pub(crate) const TAG_VG: u16 = 1965;

/// The special-element flag of a tag.
pub(crate) const SPECIAL: u16 = 0x4000;

/// One data descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dd {
    /// The tag as stored (special elements carry `0x4000`).
    pub tag: u16,
    pub rf: u16,
    pub offset: u32,
    pub length: u32,
}

impl Dd {
    pub fn base_tag(&self) -> u16 {
        self.tag & !SPECIAL
    }
    pub fn is_special(&self) -> bool {
        self.tag & SPECIAL != 0
    }
}

/// Every descriptor of the file, indexed by (base tag, reference).
pub struct Dds {
    pub list: Vec<Dd>,
    index: HashMap<(u16, u16), usize>,
}

impl Dds {
    pub(crate) fn read(data: &[u8]) -> Result<Dds> {
        let mut list = Vec::new();
        let mut index = HashMap::new();
        let mut seen = HashSet::new();
        let mut off = 4usize;
        let mut blocks = 0;
        // Blocks of a well-formed file do not overlap, so they hold at most a DD per 12 bytes;
        // overlapping blocks would otherwise rescan the same bytes 65535 times each.
        let mut budget = data.len() / 12;
        'blocks: while off != 0 {
            if !seen.insert(off) {
                break; // a cycle of DD blocks
            }
            blocks += 1;
            if blocks > limits::MAX_DD_BLOCKS {
                return Err(Error::TooLarge);
            }
            let (Some(n), Some(next)) = (be16(data, off), be32(data, off + 2)) else {
                if list.is_empty() {
                    return Err(Error::Malformed("DD block outside the file".into()));
                }
                break;
            };
            let base = off + 6;
            for k in 0..usize::from(n) {
                let p = base + 12 * k;
                let Some(b) = data.get(p..p + 12) else {
                    break;
                };
                if budget == 0 {
                    break 'blocks;
                }
                budget -= 1;
                let dd = Dd {
                    tag: u16::from_be_bytes([b[0], b[1]]),
                    rf: u16::from_be_bytes([b[2], b[3]]),
                    offset: u32::from_be_bytes([b[4], b[5], b[6], b[7]]),
                    length: u32::from_be_bytes([b[8], b[9], b[10], b[11]]),
                };
                if dd.tag == TAG_NULL || dd.tag == 0 {
                    continue;
                }
                if list.len() >= limits::MAX_DDS {
                    return Err(Error::TooLarge);
                }
                index.entry((dd.base_tag(), dd.rf)).or_insert(list.len());
                list.push(dd);
            }
            off = next as usize;
        }
        if list.is_empty() {
            return Err(Error::Malformed("no data descriptors".into()));
        }
        Ok(Dds { list, index })
    }

    /// The descriptor of (tag, ref), the special flag ignored in the lookup.
    pub fn get(&self, tag: u16, rf: u16) -> Option<&Dd> {
        self.index
            .get(&(tag & !SPECIAL, rf))
            .map(|&i| &self.list[i])
    }

    /// Whether an element exists.
    pub fn has(&self, tag: u16, rf: u16) -> bool {
        self.get(tag, rf).is_some()
    }

    /// The bytes of an element as stored (no special-element handling); `None` when absent
    /// or outside the file.
    pub fn raw<'a>(&self, data: &'a [u8], tag: u16, rf: u16) -> Option<&'a [u8]> {
        let dd = self.get(tag, rf)?;
        raw_of(data, dd)
    }

    /// The first reference of a tag.
    pub fn first_ref(&self, tag: u16) -> Option<u16> {
        self.list.iter().find(|d| d.base_tag() == tag).map(|d| d.rf)
    }

    /// Every reference of a tag in file order.
    pub fn refs(&self, tag: u16) -> impl Iterator<Item = u16> + '_ {
        self.list
            .iter()
            .filter(move |d| d.base_tag() == tag)
            .map(|d| d.rf)
    }

    /// Number of elements with this tag.
    pub fn count(&self, tag: u16) -> usize {
        self.list.iter().filter(|d| d.base_tag() == tag).count()
    }
}

pub(crate) fn raw_of<'a>(data: &'a [u8], dd: &Dd) -> Option<&'a [u8]> {
    if dd.offset == u32::MAX || dd.length == u32::MAX {
        return None;
    }
    let s = dd.offset as usize;
    data.get(s..s.checked_add(dd.length as usize)?)
}
