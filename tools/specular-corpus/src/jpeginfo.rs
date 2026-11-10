//! Tiny JPEG header parser used to record the *actual* properties of each generated file in the
//! manifest (rather than what we asked the encoder for), and to locate segments for the
//! broken-file mutations.

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct HeaderInfo {
    pub sof: String,
    pub precision: u8,
    pub width: u32,
    pub height: u32,
    pub components: u8,
    pub sampling: String,
    pub progressive: bool,
    pub arithmetic: bool,
    pub restart_interval: u16,
    pub colour_model: String,
    pub has_exif: bool,
    pub has_icc: bool,
}

/// A marker segment: marker byte, offset of the 0xFF, total length including marker.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub marker: u8,
    pub offset: usize,
    pub len: usize,
}

/// Segments from SOI up to and including the first SOS header (not the entropy data).
pub fn segments(d: &[u8]) -> Option<Vec<Segment>> {
    if d.len() < 4 || d[0] != 0xFF || d[1] != 0xD8 {
        return None;
    }
    let mut v = vec![Segment {
        marker: 0xD8,
        offset: 0,
        len: 2,
    }];
    let mut i = 2;
    while i + 4 <= d.len() {
        if d[i] != 0xFF {
            return None;
        }
        let m = d[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        let l = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        v.push(Segment {
            marker: m,
            offset: i,
            len: l + 2,
        });
        if m == 0xDA {
            return Some(v);
        }
        i += l + 2;
    }
    None
}

pub fn parse(d: &[u8]) -> Option<HeaderInfo> {
    let segs = segments(d)?;
    let mut hi = HeaderInfo::default();
    let mut adobe_transform: Option<u8> = None;
    let mut jfif = false;
    let mut samp = Vec::new();
    let mut have_sof = false;
    for s in &segs[1..] {
        let body = d.get(s.offset + 4..s.offset + s.len)?;
        match s.marker {
            0xC0..=0xCF if s.marker != 0xC4 && s.marker != 0xC8 && s.marker != 0xCC => {
                have_sof = true;
                hi.sof = format!("SOF{}", s.marker - 0xC0);
                hi.progressive = matches!(s.marker, 0xC2 | 0xC6 | 0xCA | 0xCE);
                hi.arithmetic = s.marker >= 0xC9;
                hi.precision = body[0];
                hi.height = u16::from_be_bytes([body[1], body[2]]) as u32;
                hi.width = u16::from_be_bytes([body[3], body[4]]) as u32;
                hi.components = body[5];
                for c in 0..hi.components as usize {
                    let f = *body.get(6 + c * 3 + 1)?;
                    samp.push((f >> 4, f & 15));
                }
            }
            0xDD => hi.restart_interval = u16::from_be_bytes([body[0], body[1]]),
            0xE0 if body.starts_with(b"JFIF\0") => jfif = true,
            0xE1 if body.starts_with(b"Exif\0") => hi.has_exif = true,
            0xE2 if body.starts_with(b"ICC_PROFILE\0") => hi.has_icc = true,
            0xEE if body.starts_with(b"Adobe") && body.len() >= 12 => {
                adobe_transform = Some(body[11])
            }
            _ => {}
        }
    }
    if !have_sof {
        return None;
    }
    hi.sampling = match hi.components {
        1 => "4:0:0".into(),
        3 => {
            let (h0, v0) = samp[0];
            if samp[1] != samp[2] || samp[1] != (1, 1) {
                format!(
                    "{h0}x{v0},{}x{},{}x{}",
                    samp[1].0, samp[1].1, samp[2].0, samp[2].1
                )
            } else {
                match (h0, v0) {
                    (1, 1) => "4:4:4".into(),
                    (2, 1) => "4:2:2".into(),
                    (1, 2) => "4:4:0".into(),
                    (2, 2) => "4:2:0".into(),
                    (4, 1) => "4:1:1".into(),
                    _ => format!("{h0}x{v0},1x1,1x1"),
                }
            }
        }
        _ => samp
            .iter()
            .map(|(h, v)| format!("{h}x{v}"))
            .collect::<Vec<_>>()
            .join(","),
    };
    hi.colour_model = match (hi.components, adobe_transform) {
        (1, _) => "Grey".into(),
        (3, Some(0)) => "RGB".into(),
        (3, _) if !jfif && is_rgb_ids(d, &segs) => "RGB".into(),
        (3, _) => "YCbCr".into(),
        (4, Some(2)) => "YCCK".into(),
        (4, _) => "CMYK".into(),
        _ => "unknown".into(),
    };
    Some(hi)
}

fn is_rgb_ids(d: &[u8], segs: &[Segment]) -> bool {
    segs.iter().any(|s| {
        (0xC0..=0xCF).contains(&s.marker)
            && s.marker != 0xC4
            && d.get(s.offset + 10..s.offset + 19)
                .is_some_and(|b| b[0] == b'R' && b[3] == b'G' && b[6] == b'B')
    })
}
