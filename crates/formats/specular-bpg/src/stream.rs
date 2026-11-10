//! `hevc_data()`: NAL units in Annex B framing without the first start code, colour
//! (`nuh_layer_id` 0) and alpha (`nuh_layer_id` 1) interleaved, grouped into pictures the
//! way the reference decoder groups them.

use std::ops::Range;

/// One NAL unit: where it is, and whether it belongs to the alpha picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Nal {
    pub range: Range<usize>,
    pub alpha: bool,
}

/// One frame: its NAL units, and how many frame periods it shows for.
#[derive(Clone, Debug, Default)]
pub(crate) struct AccessUnit {
    pub nals: Vec<Nal>,
    pub duration: u32,
}

/// Splits `data[range]` into NAL units (the first has no start code) and groups them into
/// access units. A new frame starts at a parameter set, AUD or prefix SEI, or at a slice
/// starting a picture, once both the colour and (with `has_alpha`) the alpha picture of the
/// current frame have started.
pub(crate) fn access_units(data: &[u8], range: Range<usize>, has_alpha: bool) -> Vec<AccessUnit> {
    let mut out = Vec::new();
    let mut cur = AccessUnit {
        nals: Vec::new(),
        duration: 1,
    };
    let mut started = [false; 2];
    for nal in split(&data[range.clone()]) {
        let r = range.start + nal.start..range.start + nal.end;
        let n = &data[r.clone()];
        if n.len() < 2 {
            continue;
        }
        let nut = (n[0] >> 1) & 0x3f;
        let layer = ((n[0] & 1) << 5) | (n[1] >> 3);
        let alpha = has_alpha && layer == 1;
        let both = started[0] && (!has_alpha || started[1]);
        let first_slice =
            (nut <= 9 || (16..=21).contains(&nut)) && n.get(2).is_some_and(|b| b & 0x80 != 0);
        let starts_frame =
            both && ((32..=35).contains(&nut) || nut == 39 || nut >= 41 || first_slice);
        if starts_frame {
            out.push(std::mem::replace(
                &mut cur,
                AccessUnit {
                    nals: Vec::new(),
                    duration: 1,
                },
            ));
            started = [false; 2];
        }
        if first_slice {
            started[usize::from(alpha)] = true;
        }
        if nut == 39
            && !alpha
            && let Some(d) = frame_duration(n)
        {
            cur.duration = d;
        }
        cur.nals.push(Nal { range: r, alpha });
    }
    if !cur.nals.is_empty() {
        out.push(cur);
    }
    out
}

/// The NAL units of an Annex B stream whose first start code is left out, as byte ranges
/// (start codes excluded; a zero byte before a four-byte start code ends no NAL unit).
fn split(data: &[u8]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0
            && data[i + 1] == 0
            && (data[i + 2] == 1 || (data[i + 2] == 0 && data.get(i + 3) == Some(&1)))
        {
            out.push(start..i);
            let code = if data[i + 2] == 1 { 3 } else { 4 };
            i += code;
            start = i;
        } else {
            i += 1;
        }
    }
    if start < data.len() {
        out.push(start..data.len());
    }
    out.retain(|r| r.end - r.start >= 2);
    out
}

/// The `frame_duration` of a prefix SEI NAL unit carrying payload type 257.
fn frame_duration(nal: &[u8]) -> Option<u32> {
    // Remove emulation prevention bytes from the payload.
    let mut rbsp = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &b in &nal[2..] {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        rbsp.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    let mut p = 0;
    while p < rbsp.len() && rbsp[p] != 0x80 {
        let mut ty = 0u32;
        while *rbsp.get(p)? == 0xff {
            ty += 255;
            p += 1;
        }
        ty += u32::from(*rbsp.get(p)?);
        p += 1;
        let mut size = 0usize;
        while *rbsp.get(p)? == 0xff {
            size += 255;
            p += 1;
        }
        size += usize::from(*rbsp.get(p)?);
        p += 1;
        let body = rbsp.get(p..p.checked_add(size)?)?;
        if ty == 257 && body.len() >= 2 {
            let d = u32::from(u16::from_be_bytes([body[0], body[1]]));
            return (d != 0).then_some(d);
        }
        p += size;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_group() {
        // VPS-less stream: PPS, slice (first in picture), then a second frame.
        let mut d = vec![0x44, 0x01, 0xaa]; // PPS
        d.extend_from_slice(&[0, 0, 1, 0x26, 0x01, 0x80, 0x11]); // IDR slice, first
        d.extend_from_slice(&[0, 0, 0, 1, 0x4e, 0x01, 0xff, 0x02, 0x02, 0x00, 0x05, 0x80]);
        d.extend_from_slice(&[0, 0, 1, 0x02, 0x01, 0x80, 0x22]); // TRAIL_R, first
        let aus = access_units(&d, 0..d.len(), false);
        assert_eq!(aus.len(), 2);
        assert_eq!(aus[0].nals.len(), 2);
        assert_eq!(aus[1].nals.len(), 2);
        assert_eq!(aus[1].duration, 5);
        assert_eq!(&d[aus[0].nals[1].range.clone()], &[0x26, 0x01, 0x80, 0x11]);
    }

    #[test]
    fn alpha_slices_before_colour() {
        // Alpha slice (layer 1) then colour slice of the same frame, then the next frame.
        let mut d = vec![0x26, 0x09, 0x80, 0x01];
        d.extend_from_slice(&[0, 0, 1, 0x26, 0x01, 0x80, 0x02]);
        d.extend_from_slice(&[0, 0, 1, 0x02, 0x09, 0x80, 0x03]);
        d.extend_from_slice(&[0, 0, 1, 0x02, 0x01, 0x80, 0x04]);
        let aus = access_units(&d, 0..d.len(), true);
        assert_eq!(aus.len(), 2);
        assert!(aus[0].nals[0].alpha && !aus[0].nals[1].alpha);
    }
}
