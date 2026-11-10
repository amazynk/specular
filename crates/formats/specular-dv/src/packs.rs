//! VAUX and subcode packs (SMPTE 314M 4.4, 4.5; IEC 61834-4 pack layouts): the source and
//! source control packs, and the recording date and time.

/// What the packs of a frame's first DIF sequences say.
#[derive(Default)]
pub(crate) struct Packs {
    /// The VAUX source pack (0x60), five bytes.
    pub vs: Option<[u8; 5]>,
    pub wide: bool,
    pub bottom_field_first: bool,
    pub recorded: Option<String>,
}

fn bcd(v: u8, tens_mask: u8) -> Option<u32> {
    let units = v & 0x0F;
    let tens = (v >> 4) & tens_mask;
    (units <= 9 && tens <= 9).then_some(u32::from(tens) * 10 + u32::from(units))
}

/// Recording date (pack 0x62: day, month, two-digit year).
fn date(p: &[u8; 5]) -> Option<(u32, u32, u32)> {
    let day = bcd(p[2], 0x3)?;
    let month = bcd(p[3], 0x1)?;
    let yy = bcd(p[4], 0xF)?;
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
        return None;
    }
    let year = if yy < 70 { 2000 + yy } else { 1900 + yy };
    Some((year, month, day))
}

/// Recording time (pack 0x63: frames, seconds, minutes, hours).
fn time(p: &[u8; 5]) -> Option<(u32, u32, u32)> {
    let s = bcd(p[2], 0x7)?;
    let m = bcd(p[3], 0x7)?;
    let h = bcd(p[4], 0x3)?;
    (s < 60 && m < 60 && h < 24).then_some((h, m, s))
}

/// Walks the VAUX blocks (and the subcode sync blocks) of the first two DIF sequences.
pub(crate) fn scan(frame: &[u8], seqs: usize) -> Packs {
    let mut vs = None;
    let mut vsc = None;
    let mut rdate = None;
    let mut rtime = None;
    let mut sub_date = None;
    let mut sub_time = None;
    let nseq = seqs.min(frame.len() / (150 * 80)).min(2);
    for s in 0..nseq {
        let seq = &frame[s * 150 * 80..];
        // VAUX: blocks 3..6, 15 packs of five bytes from byte 3.
        for b in 3..6 {
            let blk = &seq[b * 80..b * 80 + 80];
            for p in 0..15 {
                let at = 3 + p * 5;
                let pack: [u8; 5] = blk[at..at + 5].try_into().unwrap_or([0xFF; 5]);
                match pack[0] {
                    0x60 => vs = vs.or(Some(pack)),
                    0x61 => vsc = vsc.or(Some(pack)),
                    0x62 => rdate = rdate.or(date(&pack)),
                    0x63 => rtime = rtime.or(time(&pack)),
                    _ => {}
                }
            }
        }
        // Subcode: blocks 1..3, six sync blocks of eight bytes from byte 3 (ID, ID, FFh, pack).
        for b in 1..3 {
            let blk = &seq[b * 80..b * 80 + 80];
            for sb in 0..6 {
                let at = 3 + sb * 8 + 3;
                let pack: [u8; 5] = blk[at..at + 5].try_into().unwrap_or([0xFF; 5]);
                match pack[0] {
                    0x62 => sub_date = sub_date.or(date(&pack)),
                    0x63 => sub_time = sub_time.or(time(&pack)),
                    _ => {}
                }
            }
        }
    }
    let (wide, bff) = match vsc {
        // DISP 010: 16:9 full format. FF/FS: which field comes first.
        Some(p) => {
            let disp = p[2] & 7;
            let ff = p[3] & 0x80 != 0;
            let fs = p[3] & 0x40 != 0;
            (disp == 2 || disp == 7, !ff || fs)
        }
        None => (false, true),
    };
    let recorded = match (rdate.or(sub_date), rtime.or(sub_time)) {
        // 1970-01-01 00:00:00 is what encoders without a date write (FFmpeg's default).
        (Some((1970, 1, 1)), Some((0, 0, 0))) => None,
        (Some((y, mo, d)), Some((h, mi, s))) => {
            Some(format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}"))
        }
        _ => None,
    };
    Packs {
        vs,
        wide,
        bottom_field_first: bff,
        recorded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcd_packs() {
        assert_eq!(date(&[0x62, 0xFF, 0xC1, 0x01, 0x70]), Some((1970, 1, 1)));
        assert_eq!(date(&[0x62, 0xFF, 0xE5, 0xE8, 0x04]), Some((2004, 8, 25)));
        assert_eq!(date(&[0x62, 0xFF, 0xFF, 0xFF, 0xFF]), None);
        assert_eq!(time(&[0x63, 0xFF, 0xD9, 0xB0, 0xD3]), Some((13, 30, 59)));
    }
}
