//! Scenarist closed captions (`.scc`): a `Scenarist_SCC V1.0` line, then lines of a SMPTE
//! timecode and the CEA-608 byte pairs sent from that frame on, one pair a frame, as hex
//! words (`00:00:01;02 9420 9420 94ae …`). Timecodes are 29.97 frames a second, drop-frame
//! when written with `;`. The pairs go through the CEA-608 decoder ([`crate::cea608`]),
//! channel 1 (CC1) unless the caller asks for another channel of the first field.

use crate::cea608::{Channel, Decoder};
use crate::{Error, Result, Track};

/// The header line.
pub const MAGIC: &str = "Scenarist_SCC V1.0";

pub fn sniff(text: &str) -> bool {
    text.trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with(MAGIC)
}

/// A timecode's frame number at 29.97 frames a second (`;` or `.` before the frames:
/// drop-frame).
fn frame_of(tc: &str) -> Option<i64> {
    let drop = tc.contains([';', '.']);
    let parts: Vec<i64> = tc
        .split([':', ';', '.'])
        .map(|p| p.parse::<i64>().ok().filter(|v| *v >= 0))
        .collect::<Option<_>>()?;
    let [h, m, s, f] = parts.as_slice() else {
        return None;
    };
    if *m >= 60 || *s >= 60 || *f >= 30 {
        return None;
    }
    let minutes = h * 60 + m;
    let nominal = (minutes * 60 + s) * 30 + f;
    Some(if drop {
        // Frames 0 and 1 are skipped each minute but every tenth.
        nominal - 2 * (minutes - minutes / 10)
    } else {
        nominal
    })
}

/// A frame's start in microseconds (30000/1001 frames a second).
fn frame_us(frame: i64) -> i64 {
    (frame as f64 * 1001.0 / 30.0).round() as i64 * 1000
}

/// Parses an SCC file; `channel` is 1 or 2 (CC1, CC2; the file carries the first field).
pub fn parse(text: &str, channel: u8) -> Result<Track> {
    if !sniff(text) {
        return Err(Error::Malformed("no Scenarist_SCC header".into()));
    }
    let ch = Channel::from_code(channel.clamp(1, 2))
        .ok_or_else(|| Error::Unsupported("caption channel".into()))?;
    let mut dec = Decoder::new(ch);
    let mut next_frame = i64::MIN;
    let mut any = false;
    for l in text.lines().skip(1) {
        let l = l.trim();
        let Some((tc, words)) = l.split_once(char::is_whitespace) else {
            continue;
        };
        let Some(mut frame) = frame_of(tc.trim()) else {
            continue;
        };
        // Pairs still being sent from the line before push this one later.
        frame = frame.max(next_frame);
        for w in words.split_whitespace() {
            let Ok(v) = u16::from_str_radix(w, 16) else {
                continue;
            };
            if w.len() != 4 {
                continue;
            }
            dec.pair((v >> 8) as u8, v as u8);
            dec.frame_end(frame_us(frame + 1));
            frame += 1;
            any = true;
        }
        next_frame = frame;
    }
    if !any {
        return Err(Error::Malformed("no caption data".into()));
    }
    Ok(dec.finish(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cea608::fixture;

    fn words(pairs: &[[u8; 2]]) -> String {
        pairs
            .iter()
            .map(|p| format!("{:02x}{:02x}", p[0], p[1]))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn pop_on_caption() {
        // Resume caption loading, a preamble, text, end of caption (swap): shows "HELLO"
        // until the erase two seconds later.
        let mut load = Vec::new();
        load.extend(fixture::ctrl(0x14, 0x20));
        load.extend(fixture::pac(15, 0, false));
        load.extend(fixture::text("HELLO"));
        let swap = fixture::ctrl(0x14, 0x2f);
        let erase = fixture::ctrl(0x14, 0x2c);
        let scc = format!(
            "{MAGIC}\n\n00:00:00:00\t{}\n\n00:00:01:00\t{}\n\n00:00:03:00\t{}\n",
            words(&load),
            words(&swap),
            words(&erase)
        );
        let t = parse(&scc, 1).unwrap();
        assert_eq!(t.cues.len(), 1, "{:?}", t.cues);
        assert_eq!(t.cues[0].text().unwrap().plain_text(), "HELLO");
        let start = t.cues[0].start_us;
        assert!((1_000_000..1_100_000).contains(&start), "{start}");
        let end = t.cues[0].end_us;
        assert!((3_000_000..3_200_000).contains(&end), "{end}");
    }

    #[test]
    fn timecodes() {
        assert_eq!(frame_of("00:00:01:00"), Some(30));
        // 00:01:00;02 is the first frame of minute one in drop-frame counting.
        assert_eq!(frame_of("00:01:00;02"), Some(1800));
        assert_eq!(frame_of("00:10:00;00"), Some(17982));
        assert_eq!(frame_of("x"), None);
        assert!(parse("hello", 1).is_err());
        assert!(sniff("\u{feff}Scenarist_SCC V1.0\r\n"));
    }
}
