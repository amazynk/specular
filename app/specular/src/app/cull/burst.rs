//! Burst stacks: consecutive items shot by one camera within `cull.burst_gap` of each other
//! collapse into one stack in cull mode (DESIGN.md §6.3). The capture time (EXIF
//! DateTimeOriginal with SubSecTimeOriginal), the camera (make and model) and its body serial
//! come from the metadata readers the similar-shots search uses, hashed; no pixels are decoded.
//! The grouping itself is pure, so it is tested on its own.

use std::ops::Range;
use std::path::Path;

use specular_similar::hints::{self, Hints};

use super::group::{Item, is_raw};
use crate::app::gallery::Mark;

/// When and with what an item was shot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Capture {
    /// Capture time as written (local wall clock), ms since 1970.
    pub ms: i64,
    /// The time carries sub-seconds.
    pub subsec: bool,
    /// Hash of make + model.
    pub camera: Option<u64>,
    /// Hash of the body serial number.
    pub serial: Option<u64>,
    /// A burst id the camera wrote (Apple BurstUUID, Google BurstID), hashed.
    pub burst: Option<u64>,
}

impl Capture {
    fn from_hints(h: &Hints) -> Option<Capture> {
        Some(Capture {
            ms: h.time_ms?,
            subsec: h.subsec,
            camera: h.camera,
            serial: h.serial,
            burst: h.burst,
        })
    }
}

/// The capture of one file: its Exif (found in any container the decoders know, a CR3's
/// included), else what the head of the file says (XMP-only times).
pub(crate) fn read_capture(p: &Path) -> Option<Capture> {
    let mut h = Hints::default();
    if let Ok(f) = std::fs::File::open(p)
        && let Some(t) = specular_decode::read_exif_tiff(std::io::BufReader::new(f))
    {
        hints::read_exif(&t, &mut h);
    }
    if h.time_ms.is_none() {
        h = specular_similar::scan::hints_of(p);
    }
    Capture::from_hints(&h)
}

/// The capture of an item: its first rendered member that has one (its Exif is the quickest
/// to reach), else its RAW's.
pub(crate) fn item_capture(item: &Item) -> Option<Capture> {
    let (raw, other): (Vec<_>, Vec<_>) = item.members.iter().partition(|p| is_raw(p));
    other.into_iter().chain(raw).find_map(|p| read_capture(p))
}

/// Whether two consecutive shots belong to one burst: one camera (make, model and serial
/// where both say), and either one burst id or at most `gap_ms` apart. Without sub-seconds a
/// time is only known to the second, so the gap is rounded up to whole seconds.
pub(crate) fn same_burst(a: &Capture, b: &Capture, gap_ms: i64) -> bool {
    if a.camera != b.camera {
        return false;
    }
    if let (Some(x), Some(y)) = (a.serial, b.serial)
        && x != y
    {
        return false;
    }
    if let (Some(x), Some(y)) = (a.burst, b.burst) {
        return x == y;
    }
    let limit = if a.subsec && b.subsec {
        gap_ms
    } else {
        (gap_ms.max(1) + 999) / 1000 * 1000
    };
    (b.ms - a.ms).abs() <= limit
}

/// The runs of two or more consecutive items of one burst, in item order.
pub(crate) fn bursts(caps: &[Option<Capture>], gap_ms: i64) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for i in 1..=caps.len() {
        let joins = i < caps.len()
            && matches!((&caps[i - 1], &caps[i]), (Some(a), Some(b)) if same_burst(a, b, gap_ms));
        if !joins {
            if i - start >= 2 {
                out.push(start..i);
            }
            start = i;
        }
    }
    out
}

/// A burst in cull mode: its items, the one shown while it is collapsed, and whether it is
/// expanded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stack {
    pub items: Range<usize>,
    pub top: usize,
    pub open: bool,
}

/// The frame a collapsed burst shows: its pick, else its best rated, else its first frame
/// that is not rejected, else its first.
pub(crate) fn top(items: Range<usize>, mark: impl Fn(usize) -> Mark) -> usize {
    let start = items.start;
    let marks: Vec<(usize, Mark)> = items.map(|i| (i, mark(i))).collect();
    if let Some((i, _)) = marks.iter().find(|(_, m)| m.flag == 1) {
        return *i;
    }
    let best = marks
        .iter()
        .filter(|(_, m)| m.rating > 0)
        .max_by_key(|(i, m)| (m.rating, std::cmp::Reverse(*i)));
    if let Some((i, _)) = best {
        return *i;
    }
    marks
        .iter()
        .find(|(_, m)| m.flag != -1)
        .map_or(start, |(i, _)| *i)
}

/// The stack holding item `i`.
pub(crate) fn stack_of(stacks: &[Stack], i: usize) -> Option<usize> {
    stacks.iter().position(|s| s.items.contains(&i))
}

/// The items on screen, in order: every item, except that a collapsed stack shows its top
/// alone.
pub(crate) fn visible(n: usize, stacks: &[Stack]) -> Vec<usize> {
    let mut out = Vec::with_capacity(n);
    let mut s = stacks.iter().peekable();
    let mut i = 0;
    while i < n {
        if let Some(st) = s.peek()
            && st.items.start == i
        {
            if st.open {
                out.extend(st.items.clone());
            } else {
                out.push(st.top);
            }
            i = st.items.end.max(i + 1);
            s.next();
            continue;
        }
        out.push(i);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: i64) -> Option<Capture> {
        Some(Capture {
            ms,
            subsec: true,
            camera: Some(1),
            serial: Some(7),
            burst: None,
        })
    }

    #[test]
    fn consecutive_shots_within_the_gap_stack() {
        // 10 fps, a pause, 3 more, one alone, then two without a time.
        let caps = vec![
            at(0),
            at(100),
            at(200),
            at(5_000),
            at(5_400),
            at(6_300),
            at(20_000),
            None,
            None,
        ];
        assert_eq!(bursts(&caps, 1000), vec![0..3, 3..6]);
        // A tighter gap splits the second run.
        assert_eq!(bursts(&caps, 500), vec![0..3, 3..5]);
        assert!(bursts(&[], 1000).is_empty());
        assert!(bursts(&[at(0)], 1000).is_empty());
    }

    #[test]
    fn another_body_or_model_breaks_a_burst() {
        let mut other_body = at(100).unwrap();
        other_body.serial = Some(8);
        let mut other_model = at(100).unwrap();
        other_model.camera = Some(2);
        let mut no_serial = at(100).unwrap();
        no_serial.serial = None;
        let a = at(0).unwrap();
        assert!(!same_burst(&a, &other_body, 1000));
        assert!(!same_burst(&a, &other_model, 1000));
        // A serial written by only one of them does not decide.
        assert!(same_burst(&a, &no_serial, 1000));
    }

    #[test]
    fn whole_seconds_and_burst_ids() {
        let sec = |ms| Capture {
            ms,
            subsec: false,
            camera: Some(1),
            serial: None,
            burst: None,
        };
        // Known to the second: the next second may be 1 ms later.
        assert!(same_burst(&sec(0), &sec(1000), 1000));
        assert!(!same_burst(&sec(0), &sec(2000), 1000));
        assert!(same_burst(&sec(0), &sec(1000), 300));
        // Name order may run against time.
        assert!(same_burst(&sec(1000), &sec(0), 1000));
        // One burst id groups whatever the gap; two different ones never.
        let id = |ms, b| Capture {
            burst: Some(b),
            ..sec(ms)
        };
        assert!(same_burst(&id(0, 5), &id(9000, 5), 1000));
        assert!(!same_burst(&id(0, 5), &id(0, 6), 1000));
    }

    #[test]
    fn top_prefers_pick_then_stars_then_not_rejected() {
        let m = |flag: i8, rating: u8| Mark {
            rating,
            label: 0,
            flag,
        };
        let marks = [m(-1, 0), m(0, 2), m(1, 0), m(0, 4)];
        assert_eq!(top(0..4, |i| marks[i]), 2);
        assert_eq!(top(0..2, |i| marks[i]), 1);
        assert_eq!(top(3..4, |i| marks[i]), 3);
        let none = [m(-1, 0), m(0, 0), m(0, 0)];
        assert_eq!(top(0..3, |i| none[i]), 1);
        let rejected = [m(-1, 0), m(-1, 0)];
        assert_eq!(top(0..2, |i| rejected[i]), 0);
        // Equal stars: the first.
        let tie = [m(0, 3), m(0, 3)];
        assert_eq!(top(0..2, |i| tie[i]), 0);
    }

    /// A little-endian TIFF: IFD0 (make, model, Exif pointer) and an Exif IFD (capture time,
    /// sub-seconds, body serial). Made-up values only.
    fn tiff(model: &str, time: &str, sub: &str, serial: &str) -> Vec<u8> {
        fn ascii(s: &str) -> Vec<u8> {
            let mut v = s.as_bytes().to_vec();
            v.push(0);
            v
        }
        // (tag, type, count, inline value or out-of-line bytes)
        fn ifd(out: &mut Vec<u8>, entries: &[(u16, u16, Vec<u8>)], next_data: &mut Vec<u8>) {
            let start = out.len();
            let data_at = start + 2 + entries.len() * 12 + 4;
            out.extend((entries.len() as u16).to_le_bytes());
            let mut data = Vec::new();
            for (tag, ty, bytes) in entries {
                out.extend(tag.to_le_bytes());
                out.extend(ty.to_le_bytes());
                let count = if *ty == 2 { bytes.len() } else { 1 };
                out.extend((count as u32).to_le_bytes());
                if bytes.len() <= 4 {
                    let mut b = bytes.clone();
                    b.resize(4, 0);
                    out.extend(b);
                } else {
                    out.extend(((data_at + data.len()) as u32).to_le_bytes());
                    data.extend(bytes);
                    if data.len() % 2 == 1 {
                        data.push(0);
                    }
                }
            }
            out.extend(0u32.to_le_bytes());
            out.extend(&data);
            *next_data = data;
        }
        let mut out = b"II*\0".to_vec();
        out.extend(8u32.to_le_bytes());
        let mut scratch = Vec::new();
        // IFD0's size is known up front: 3 entries, then make and model.
        let make = ascii("Synthetic");
        let model = ascii(model);
        let ifd0_len = 2 + 3 * 12 + 4 + make.len().div_ceil(2) * 2 + model.len().div_ceil(2) * 2;
        let exif_at = (8 + ifd0_len) as u32;
        ifd(
            &mut out,
            &[
                (0x010F, 2, make),
                (0x0110, 2, model),
                (0x8769, 4, exif_at.to_le_bytes().to_vec()),
            ],
            &mut scratch,
        );
        assert_eq!(out.len() as u32, exif_at);
        ifd(
            &mut out,
            &[
                (0x9003, 2, ascii(time)),
                (0x9291, 2, ascii(sub)),
                (0xA431, 2, ascii(serial)),
            ],
            &mut scratch,
        );
        out
    }

    /// A JPEG holding only an Exif segment (no pixels are read for captures).
    fn jpeg(tiff: &[u8]) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8, 0xFF, 0xE1];
        v.extend(((tiff.len() + 8) as u16).to_be_bytes());
        v.extend(b"Exif\0\0");
        v.extend(tiff);
        v.extend([0xFF, 0xD9]);
        v
    }

    #[test]
    fn captures_come_from_exif_and_group_bursts() {
        let d = std::env::var_os("SPECULAR_TEST_TMP")
            .map_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-tmp"),
                std::path::PathBuf::from,
            )
            .join(format!("cull-burst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let shots = [
            ("A_1.JPG", "Model X", "2026:05:04 10:00:00", "10", "SN-A"),
            ("A_2.JPG", "Model X", "2026:05:04 10:00:00", "35", "SN-A"),
            ("A_3.JPG", "Model X", "2026:05:04 10:00:01", "05", "SN-A"),
            // Another body of the same model, at once.
            ("B_1.JPG", "Model X", "2026:05:04 10:00:01", "06", "SN-B"),
            ("C_1.JPG", "Model X", "2026:05:04 10:00:09", "00", "SN-A"),
        ];
        for (f, model, time, sub, serial) in shots {
            std::fs::write(d.join(f), jpeg(&tiff(model, time, sub, serial))).unwrap();
        }
        let caps: Vec<Option<Capture>> = shots.iter().map(|s| read_capture(&d.join(s.0))).collect();
        let a1 = caps[0].unwrap();
        assert!(a1.subsec);
        assert_eq!(caps[1].unwrap().ms - a1.ms, 250);
        assert_eq!(caps[2].unwrap().ms - a1.ms, 950);
        assert!(a1.camera.is_some() && a1.serial.is_some());
        assert_ne!(a1.serial, caps[3].unwrap().serial);
        assert_eq!(bursts(&caps, 1000), vec![0..3]);
        assert_eq!(bursts(&caps, 500), vec![0..2]);
        // An item reads its rendered member; a file without Exif has no capture.
        std::fs::write(d.join("plain.jpg"), [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
        assert!(read_capture(&d.join("plain.jpg")).is_none());
        let item = Item {
            shown: d.join("A_2.JPG"),
            members: vec![d.join("plain.jpg"), d.join("A_2.JPG")],
        };
        assert_eq!(item_capture(&item), caps[1]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn collapsed_stacks_show_their_top() {
        let stacks = vec![
            Stack {
                items: 1..4,
                top: 2,
                open: false,
            },
            Stack {
                items: 5..7,
                top: 5,
                open: true,
            },
        ];
        assert_eq!(visible(8, &stacks), vec![0, 2, 4, 5, 6, 7]);
        assert_eq!(visible(3, &[]), vec![0, 1, 2]);
        assert_eq!(stack_of(&stacks, 3), Some(0));
        assert_eq!(stack_of(&stacks, 4), None);
        assert_eq!(stack_of(&stacks, 6), Some(1));
    }
}
