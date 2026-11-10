//! Leaf "PKTS" metadata (TIFF tag 0x8606 of MOS files): nested records of a 40-byte name, a
//! big-endian size and ASCII (or nested record) data.

use std::ops::Range;

/// Facts read from the PKTS records.
#[derive(Clone, Debug, Default)]
pub(crate) struct LeafInfo {
    /// Camera multipliers R, G, B (neutral grey over each channel's value).
    pub wb: Option<[f32; 3]>,
    /// Raster index (0..4) of red in the 2x2 pattern.
    pub red_at: Option<usize>,
    /// Image rotation in degrees after the raw data rotation.
    pub rotation: Option<i32>,
    /// The digital back's model, e.g. "Aptus 22".
    pub back: Option<&'static str>,
    pub preview: Option<Range<usize>>,
}

/// Back names by `ShootObj_back_type`.
const BACKS: [&str; 18] = [
    "",
    "DCB2",
    "Volare",
    "Cantare",
    "CMost",
    "Valeo 6",
    "Valeo 11",
    "Valeo 22",
    "Valeo 11p",
    "Valeo 17",
    "",
    "Aptus 17",
    "Aptus 22",
    "Aptus 75",
    "Aptus 65",
    "Aptus 54S",
    "Aptus 65S",
    "Aptus 75S",
];

fn ints(b: &[u8]) -> Vec<i64> {
    String::from_utf8_lossy(b)
        .split(|c: char| c.is_ascii_whitespace() || c == '\0')
        .filter_map(|t| t.parse::<f64>().ok())
        .map(|v| v as i64)
        .collect()
}

/// Reads the records in `d[range]` (nested records up to a small depth).
pub(crate) fn pkts(d: &[u8], range: Range<usize>) -> LeafInfo {
    let mut info = LeafInfo::default();
    let mut raw_rotation = 0;
    let mut budget = 100_000usize;
    walk(d, range, 0, &mut info, &mut raw_rotation, &mut budget);
    if let Some(r) = info.rotation.as_mut() {
        *r -= raw_rotation;
    }
    info
}

fn walk(
    d: &[u8],
    range: Range<usize>,
    depth: u32,
    info: &mut LeafInfo,
    raw_rot: &mut i32,
    budget: &mut usize,
) {
    let end = range.end.min(d.len());
    let mut p = range.start;
    while p.checked_add(52).is_some_and(|e| e <= end) && &d[p..p + 4] == b"PKTS" {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let name = &d[p + 8..p + 48];
        let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(40)];
        let size = u32::from_be_bytes([d[p + 48], d[p + 49], d[p + 50], d[p + 51]]) as usize;
        let from = p + 52;
        let to = from.saturating_add(size).min(end);
        let data = &d[from..to];
        match name {
            b"CaptProf_raw_data_rotation" => {
                *raw_rot = ints(data).first().copied().unwrap_or(0) as i32;
            }
            b"ImgProf_rotation_angle" => {
                info.rotation = ints(data).first().map(|&v| v as i32);
            }
            b"CaptProf_mosaic_pattern" => {
                if let Some(c) = ints(data).iter().take(4).position(|&v| v == 1) {
                    info.red_at = Some(c ^ (c >> 1));
                }
            }
            b"NeutObj_neutrals" if info.wb.is_none() => {
                let n = ints(data);
                if n.len() >= 4 && n[..4].iter().all(|&v| v > 0) {
                    let g = n[0] as f32;
                    info.wb = Some([g / n[1] as f32, g / n[2] as f32, g / n[3] as f32]);
                }
            }
            b"ShootObj_back_type" => {
                if let Some(&i) = ints(data).first()
                    && let Some(&b) = BACKS.get(i as usize)
                    && !b.is_empty()
                {
                    info.back = Some(b);
                }
            }
            b"JPEG_preview_data" => info.preview = Some(from..to),
            _ => {}
        }
        if depth < 8 && data.starts_with(b"PKTS") {
            walk(d, from..to, depth + 1, info, raw_rot, budget);
        }
        p = to;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, data: &[u8]) -> Vec<u8> {
        let mut v = b"PKTS\0\0\0\x01".to_vec();
        let mut n = [0u8; 40];
        n[..name.len()].copy_from_slice(name.as_bytes());
        v.extend_from_slice(&n);
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn reads_nested_records() {
        let inner = [
            rec("NeutObj_neutrals", b"3270\n2703\n3270\n2232\0"),
            rec("ShootObj_back_type", b"12\0"),
        ]
        .concat();
        let d = [
            rec("camera_profile", &inner),
            rec("CaptProf_mosaic_pattern", b"1\n3\n2\n0\0"),
            rec("ImgProf_rotation_angle", b"90\0"),
        ]
        .concat();
        let i = pkts(&d, 0..d.len());
        let wb = i.wb.unwrap();
        assert!((wb[0] - 1.2098).abs() < 1e-3 && (wb[2] - 1.4651).abs() < 1e-3);
        assert_eq!(i.back, Some("Aptus 22"));
        assert_eq!(i.red_at, Some(0));
        assert_eq!(i.rotation, Some(90));
        // Truncated and hostile sizes stop cleanly.
        for n in 0..d.len() {
            let _ = pkts(&d[..n], 0..d.len());
        }
        let mut bad = rec("x", b"");
        bad[48..52].copy_from_slice(&u32::MAX.to_be_bytes());
        let _ = pkts(&bad, 0..usize::MAX);
    }
}
