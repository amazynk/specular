//! Standard MIDI Files (formats 0, 1 and 2; `.mid`, `.midi`, `.kar`) and RIFF MIDI (`.rmi`,
//! RIFF `RMID` with the file in its `data` chunk), from the MIDI Association's SMF 1.0
//! specification: tracks of delta-timed events with running status, merged into one list
//! timed in microseconds through the tempo map (or SMPTE time).
//!
//! Text kept: the first track's name (the title), copyright, the first text events (the
//! comment), and karaoke lyrics (`.kar`: text events with `/` and `\` line breaks, `@`
//! lines naming the song).

use crate::{Result, bad};

/// A channel event's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    NoteOn {
        key: u8,
        vel: u8,
    },
    NoteOff {
        key: u8,
    },
    Control {
        cc: u8,
        value: u8,
    },
    Program(u8),
    /// Pitch bend, -8192..8191.
    Bend(i16),
    /// A General MIDI, GS or XG reset (system exclusive).
    Reset,
}

/// A timed event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// Microseconds from the start.
    pub us: u64,
    pub channel: u8,
    pub kind: Kind,
}

/// A song: its events in time order and what its text says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub format: u16,
    pub tracks: u16,
    pub events: Vec<Event>,
    /// Microseconds until the last event.
    pub length_us: u64,
    /// Fields by the sound tags' keys ("title", "copyright", "comment").
    pub info: Vec<(&'static str, String)>,
    /// Karaoke or lyric lines, with the microsecond each starts.
    pub lyrics: Vec<(u64, String)>,
    pub notes: usize,
    /// Channels that play notes.
    pub channels_used: u16,
}

/// Whether `d` is a Standard MIDI File or a RIFF MIDI file.
pub fn is_midi(d: &[u8]) -> bool {
    d.starts_with(b"MThd") && d.len() >= 14 || is_rmid(d)
}

pub fn is_rmid(d: &[u8]) -> bool {
    d.len() >= 20 && &d[..4] == b"RIFF" && &d[8..12] == b"RMID"
}

/// The Standard MIDI File inside a RIFF MIDI file (or `d` itself), and an embedded DLS
/// bank's bytes when the RIFF file carries one.
pub fn unwrap(d: &[u8]) -> (&[u8], Option<std::ops::Range<usize>>) {
    if !is_rmid(d) {
        return (d, None);
    }
    let cs = crate::chunks(d, 12, 8 + crate::u32le(d, 4) as usize);
    let smf = cs
        .iter()
        .find(|(id, _)| id == b"data")
        .map(|(_, r)| &d[r.clone()])
        .unwrap_or(&[]);
    // DLS level 2 puts a whole "RIFF DLS " chunk inside.
    let dls = cs
        .iter()
        .find(|(id, r)| id == b"RIFF" && d.get(r.start..r.start + 4) == Some(b"DLS "))
        .map(|(_, r)| r.start - 8..r.end);
    (smf, dls)
}

fn vlq(d: &[u8], at: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    for _ in 0..4 {
        let b = *d.get(*at)?;
        *at += 1;
        v = (v << 7) | u32::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    Some(v)
}

/// The most events read (about 40 MB of them).
const MAX_EVENTS: usize = 4 << 20;

/// An event at a tick, before timing: (tick, order, channel, kind); tempo changes are
/// `channel` 0xFF with the tempo in `kind`'s place.
#[derive(Clone, Copy)]
struct Raw {
    tick: u64,
    track: u16,
    seq: u32,
    channel: u8,
    kind: Option<Kind>,
    tempo: u32,
}

pub fn parse(d: &[u8]) -> Result<Song> {
    let (d, _) = unwrap(d);
    if !d.starts_with(b"MThd") || d.len() < 14 {
        return Err(bad("not a MIDI file"));
    }
    let hlen = u32::from_be_bytes(d[4..8].try_into().unwrap_or([0; 4])) as usize;
    let format = u16::from_be_bytes([d[8], d[9]]);
    let ntracks = u16::from_be_bytes([d[10], d[11]]);
    let division = u16::from_be_bytes([d[12], d[13]]);
    if format > 2 {
        return Err(crate::Error::Unsupported(format!(
            "MIDI file format {format}"
        )));
    }
    if division == 0 {
        return Err(bad("MIDI file without a time division"));
    }
    let mut at = 8usize.saturating_add(hlen);
    let mut raw: Vec<Raw> = Vec::new();
    let mut song = Song {
        format,
        tracks: ntracks,
        ..Song::default()
    };
    let mut texts: Vec<(u64, u16, String)> = Vec::new();
    let mut lyric_events: Vec<(u64, u16, u32, String)> = Vec::new();
    let mut kar = false;
    let mut offset_tick = 0u64; // format 2: tracks follow each other
    let mut track = 0u16;
    let mut seq = 0u32;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let len = u32::from_be_bytes(d[at + 4..at + 8].try_into().unwrap_or([0; 4])) as usize;
        let body = at + 8;
        let end = body.saturating_add(len).min(d.len());
        at = body.saturating_add(len);
        if id != b"MTrk" {
            continue;
        }
        let t = &d[body..end];
        let mut p = 0usize;
        let mut tick = offset_tick;
        let mut status = 0u8;
        while p < t.len() && raw.len() < MAX_EVENTS {
            let Some(delta) = vlq(t, &mut p) else { break };
            tick += u64::from(delta);
            let Some(&first) = t.get(p) else { break };
            if first & 0x80 != 0 {
                p += 1;
                if first < 0xF0 {
                    status = first;
                }
            } else if status == 0 {
                break;
            }
            let s = if first & 0x80 != 0 { first } else { status };
            match s {
                0xFF => {
                    status = 0;
                    let Some(&kind) = t.get(p) else { break };
                    p += 1;
                    let Some(n) = vlq(t, &mut p) else { break };
                    let n = n as usize;
                    let Some(data) = t.get(p..p + n) else { break };
                    p += n;
                    match kind {
                        0x51 if n == 3 => {
                            let tempo = u32::from_be_bytes([0, data[0], data[1], data[2]]);
                            raw.push(Raw {
                                tick,
                                track,
                                seq,
                                channel: 0xFF,
                                kind: None,
                                tempo: tempo.max(1),
                            });
                            seq += 1;
                        }
                        0x2F => break,
                        0x01..=0x07 => {
                            let s = crate::text(data);
                            if kind == 0x01 && s.starts_with("@KMIDI KARAOKE") {
                                kar = true;
                            }
                            if kind == 0x05 || kind == 0x01 {
                                lyric_events.push((tick, track, seq, s.clone()));
                                seq += 1;
                            }
                            texts.push((tick, u16::from(kind) | (track << 8), s));
                        }
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    status = 0;
                    let Some(n) = vlq(t, &mut p) else { break };
                    let n = n as usize;
                    let Some(data) = t.get(p..p + n) else { break };
                    p += n;
                    // GM on (7E 7F 09 01), GS reset (41 .. 42 12 40 00 7F 00), XG on
                    // (43 1x 4C 00 00 7E 00).
                    let reset = data.starts_with(&[0x7E, 0x7F, 0x09, 0x01])
                        || data.starts_with(&[0x7E, 0x7F, 0x09, 0x03])
                        || (data.first() == Some(&0x41)
                            && data.get(2..8) == Some(&[0x42, 0x12, 0x40, 0x00, 0x7F, 0x00]))
                        || (data.first() == Some(&0x43)
                            && data.get(2..7) == Some(&[0x4C, 0x00, 0x00, 0x7E, 0x00]));
                    if reset {
                        raw.push(Raw {
                            tick,
                            track,
                            seq,
                            channel: 0,
                            kind: Some(Kind::Reset),
                            tempo: 0,
                        });
                        seq += 1;
                    }
                }
                0xF1..=0xFE => {
                    // System common/real-time in a file: skip its data bytes.
                    p += match s {
                        0xF2 => 2,
                        0xF1 | 0xF3 => 1,
                        _ => 0,
                    };
                }
                _ => {
                    let need = if matches!(s & 0xF0, 0xC0 | 0xD0) {
                        1
                    } else {
                        2
                    };
                    let Some(b) = t.get(p..p + need) else { break };
                    p += need;
                    let ch = s & 0x0F;
                    let (a, v) = (b[0] & 0x7F, b.get(1).map_or(0, |v| v & 0x7F));
                    let kind = match s & 0xF0 {
                        0x80 => Some(Kind::NoteOff { key: a }),
                        0x90 if v == 0 => Some(Kind::NoteOff { key: a }),
                        0x90 => Some(Kind::NoteOn { key: a, vel: v }),
                        0xB0 => Some(Kind::Control { cc: a, value: v }),
                        0xC0 => Some(Kind::Program(a)),
                        0xE0 => Some(Kind::Bend(((i16::from(v) << 7) | i16::from(a)) - 8192)),
                        _ => None,
                    };
                    if let Some(k) = kind {
                        if let Kind::NoteOn { .. } = k {
                            song.notes += 1;
                            song.channels_used |= 1 << ch;
                        }
                        raw.push(Raw {
                            tick,
                            track,
                            seq,
                            channel: ch,
                            kind: Some(k),
                            tempo: 0,
                        });
                        seq += 1;
                    }
                }
            }
        }
        if format == 2 {
            offset_tick = tick;
        }
        track += 1;
    }
    if track == 0 {
        return Err(bad("MIDI file without tracks"));
    }
    // Merge by tick; within a tick keep each track's order, tempo first.
    raw.sort_by_key(|r| (r.tick, r.channel != 0xFF, r.track, r.seq));
    let smpte = division & 0x8000 != 0;
    let us_per_tick_smpte = if smpte {
        let fps = match (division >> 8) as u8 as i8 {
            -24 => 24.0,
            -25 => 25.0,
            -29 => 29.97,
            _ => 30.0,
        };
        let tpf = f64::from(division & 0xFF).max(1.0);
        1e6 / (fps * tpf)
    } else {
        0.0
    };
    let tpq = f64::from(division & 0x7FFF).max(1.0);
    let mut tempo = 500_000.0f64;
    let mut last_tick = 0u64;
    let mut last_us = 0.0f64;
    // A tempo map of (tick, µs, tempo) for timing the text too.
    let mut map: Vec<(u64, f64, f64)> = vec![(0, 0.0, tempo)];
    let to_us = |tick: u64, tempo: f64, last_tick: &mut u64, last_us: &mut f64| -> f64 {
        let dt = tick.saturating_sub(*last_tick) as f64;
        *last_us += if smpte {
            dt * us_per_tick_smpte
        } else {
            dt * tempo / tpq
        };
        *last_tick = tick;
        *last_us
    };
    let mut events = Vec::with_capacity(raw.len());
    for r in &raw {
        let us = to_us(r.tick, tempo, &mut last_tick, &mut last_us);
        if r.channel == 0xFF {
            tempo = f64::from(r.tempo);
            map.push((r.tick, us, tempo));
            continue;
        }
        if let Some(kind) = r.kind {
            events.push(Event {
                us: us as u64,
                channel: r.channel,
                kind,
            });
        }
    }
    let time_of = |tick: u64| -> u64 {
        let i = map.partition_point(|m| m.0 <= tick).saturating_sub(1);
        let (t0, u0, tp) = map[i];
        let dt = (tick - t0.min(tick)) as f64;
        (u0 + if smpte {
            dt * us_per_tick_smpte
        } else {
            dt * tp / tpq
        }) as u64
    };
    song.length_us = events.last().map_or(0, |e| e.us);
    song.events = events;
    // Text: the first track's name is the title (format 1's first track names the song).
    let first = |kind: u16| {
        texts
            .iter()
            .find(|(_, k, s)| k & 0xFF == kind && !s.is_empty())
            .map(|(_, _, s)| s.clone())
    };
    let title = texts
        .iter()
        .find(|(_, k, s)| *k == 0x03 && !s.is_empty())
        .map(|(_, _, s)| s.clone())
        .or_else(|| first(0x03));
    // Karaoke files name the song in "@T" lines.
    let kar_title = texts
        .iter()
        .find(|(_, k, s)| k & 0xFF == 1 && s.starts_with("@T"))
        .map(|(_, _, s)| s[2..].trim().to_owned());
    if let Some(t) = kar_title.or(title) {
        song.info.push(("title", t));
    }
    if let Some(c) = first(0x02) {
        song.info.push(("copyright", c));
    }
    if !kar && let Some(c) = first(0x01) {
        song.info.push(("comment", c));
    }
    // Lyrics: karaoke text events (or lyric meta events), joined into lines at '/' and '\'.
    lyric_events.sort_by_key(|e| (e.0, e.1, e.2));
    let has_lyric_meta = texts.iter().any(|(_, k, _)| k & 0xFF == 5);
    let mut line = String::new();
    let mut line_start = None;
    for (tick, track, _, s) in &lyric_events {
        let is_meta5 = texts
            .iter()
            .any(|(t, k, x)| t == tick && *k == (5 | (track << 8)) && x == s);
        if kar {
            if s.starts_with('@') || is_meta5 {
                continue;
            }
        } else if !(has_lyric_meta && is_meta5) {
            continue;
        }
        let us = time_of(*tick);
        let (brk, rest) = match s.chars().next() {
            Some('/') | Some('\\') => (true, &s[1..]),
            _ => (false, s.as_str()),
        };
        if (brk || s.ends_with('\n') || s.ends_with('\r')) && !line.trim().is_empty() {
            if let Some(t) = line_start {
                song.lyrics.push((t, line.trim().to_owned()));
            }
            line.clear();
            line_start = None;
        }
        if line_start.is_none() {
            line_start = Some(us);
        }
        line.push_str(rest.trim_end_matches(['\r', '\n']));
        if song.lyrics.len() > 10_000 {
            break;
        }
    }
    if let Some(t) = line_start
        && !line.trim().is_empty()
    {
        song.lyrics.push((t, line.trim().to_owned()));
    }
    Ok(song)
}

/// Builds a format-0 file of notes (key, start and length in ticks at 480 a quarter and
/// 120 bpm), with a track name, for tests.
#[doc(hidden)]
pub fn build(name: &str, program: u8, notes: &[(u8, u32, u32)]) -> Vec<u8> {
    let mut ev: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut meta = vec![0xFF, 0x03, name.len() as u8];
    meta.extend_from_slice(name.as_bytes());
    ev.push((0, meta));
    ev.push((0, vec![0xC0, program]));
    for &(k, start, len) in notes {
        ev.push((start, vec![0x90, k, 100]));
        ev.push((start + len, vec![0x80, k, 0]));
    }
    ev.sort_by_key(|e| e.0);
    let mut trk = Vec::new();
    let mut last = 0;
    for (t, e) in ev {
        let mut dt = t - last;
        last = t;
        let mut bytes = vec![(dt & 0x7F) as u8];
        dt >>= 7;
        while dt > 0 {
            bytes.push(0x80 | (dt & 0x7F) as u8);
            dt >>= 7;
        }
        bytes.reverse();
        trk.extend(bytes);
        trk.extend(e);
    }
    trk.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
    let mut f = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0".to_vec();
    f.extend_from_slice(b"MTrk");
    f.extend_from_slice(&(trk.len() as u32).to_be_bytes());
    f.extend(trk);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_text() {
        let f = build("Scale", 0, &[(60, 0, 480), (62, 480, 480), (64, 960, 960)]);
        assert!(is_midi(&f));
        let s = parse(&f).unwrap();
        assert_eq!(s.notes, 3);
        assert_eq!(s.info[0], ("title", "Scale".into()));
        // 120 bpm: a quarter is half a second.
        assert_eq!(s.length_us, 2_000_000);
        let on: Vec<u64> = s
            .events
            .iter()
            .filter(|e| matches!(e.kind, Kind::NoteOn { .. }))
            .map(|e| e.us)
            .collect();
        assert_eq!(on, vec![0, 500_000, 1_000_000]);
    }

    #[test]
    fn running_status_and_tempo() {
        // Tempo 1 s a quarter, a note on with running status, note off by velocity 0.
        let trk: &[u8] = &[
            0x00, 0xFF, 0x51, 0x03, 0x0F, 0x42, 0x40, 0x00, 0x90, 60, 100, 0x83, 0x60, 60, 0, 0x00,
            0xFF, 0x2F, 0x00,
        ];
        let mut f = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0MTrk".to_vec();
        f.extend_from_slice(&(trk.len() as u32).to_be_bytes());
        f.extend_from_slice(trk);
        let s = parse(&f).unwrap();
        assert_eq!(s.events.len(), 2);
        assert_eq!(s.events[1].kind, Kind::NoteOff { key: 60 });
        assert_eq!(s.events[1].us, 1_000_000);
    }

    #[test]
    fn riff_midi_unwraps() {
        let smf = build("x", 0, &[(60, 0, 10)]);
        let mut body = b"RMID".to_vec();
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(smf.len() as u32).to_le_bytes());
        body.extend_from_slice(&smf);
        let mut f = b"RIFF".to_vec();
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend(body);
        assert!(is_rmid(&f));
        assert_eq!(parse(&f).unwrap().notes, 1);
    }
}
