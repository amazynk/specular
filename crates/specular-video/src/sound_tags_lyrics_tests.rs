//! Lyrics in each kind of tag.

use super::*;

fn id3_frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend_from_slice(&(body.len() as u32).to_be_bytes());
    v.extend_from_slice(&[0, 0]);
    v.extend_from_slice(body);
    v
}

fn id3(frames: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = frames.concat();
    let n = body.len();
    let mut v = vec![b'I', b'D', b'3', 3, 0, 0];
    v.extend_from_slice(&[
        (n >> 21) as u8 & 127,
        (n >> 14) as u8 & 127,
        (n >> 7) as u8 & 127,
        n as u8 & 127,
    ]);
    v.extend(body);
    v
}

#[test]
fn id3_uslt_and_sylt() {
    // USLT: encoding, language, description, text.
    let mut uslt = vec![3, b'e', b'n', b'g'];
    uslt.extend_from_slice(b"desc\0First line\r\nSecond line\r\n");
    let t = read(&id3(&[id3_frame(b"USLT", &uslt)]));
    assert_eq!(t.lyrics.as_deref(), Some("First line\nSecond line"));
    let l = t.lyrics().unwrap();
    assert!(!l.synced);
    assert_eq!(l.lines.len(), 2);
    // SYLT next to USLT: the timed lyrics win.
    let mut sylt = vec![3, b'e', b'n', b'g', 2, 1, 0];
    for (text, ms) in [("One", 1000u32), ("Two", 2000)] {
        sylt.extend_from_slice(text.as_bytes());
        sylt.push(0);
        sylt.extend_from_slice(&ms.to_be_bytes());
    }
    let t = read(&id3(&[
        id3_frame(b"USLT", &uslt),
        id3_frame(b"SYLT", &sylt),
    ]));
    let l = t.lyrics().unwrap();
    assert!(l.synced);
    assert_eq!(l.lines[1].text, "Two");
    assert_eq!(l.lines[1].at, Some(2.0));
    // LRC in USLT is timed too.
    let mut lrc = vec![3, b'e', b'n', b'g', 0];
    lrc.extend_from_slice(b"[00:05.00]Timed");
    let t = read(&id3(&[id3_frame(b"USLT", &lrc)]));
    assert_eq!(t.lyrics().unwrap().lines[0].at, Some(5.0));
    // No lyrics: none.
    assert_eq!(read(&id3(&[])).lyrics(), None);
}

#[test]
fn vorbis_lyrics_and_unsynced() {
    for key in ["LYRICS", "UNSYNCEDLYRICS", "lyrics"] {
        let mut vc = Vec::new();
        vc.extend_from_slice(&0u32.to_le_bytes());
        let c = format!("{key}=La la\nla");
        vc.extend_from_slice(&1u32.to_le_bytes());
        vc.extend_from_slice(&(c.len() as u32).to_le_bytes());
        vc.extend_from_slice(c.as_bytes());
        let mut t = Tags::default();
        vorbis_comments(&vc, &mut t);
        assert_eq!(t.lyrics.as_deref(), Some("La la\nla"), "{key}");
        // Not a field shown in the list.
        assert!(t.fields.is_empty());
    }
}

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

#[test]
fn mp4_lyr_item() {
    let mut data = vec![0, 0, 0, 1, 0, 0, 0, 0];
    data.extend_from_slice(b"[00:01.00]Hi\n[00:02.00]there");
    let item = mp4_box(b"\xa9lyr", &mp4_box(b"data", &data));
    let ilst = mp4_box(b"ilst", &item);
    let mut meta_body = vec![0, 0, 0, 0];
    meta_body.extend(ilst);
    let moov = mp4_box(b"moov", &mp4_box(b"udta", &mp4_box(b"meta", &meta_body)));
    let mut d = mp4_box(b"ftyp", b"M4A \0\0\0\0");
    d.extend(moov);
    let t = read(&d);
    let l = t.lyrics().unwrap();
    assert!(l.synced);
    assert_eq!(l.lines.len(), 2);
}

#[test]
fn ape_lyrics_item() {
    let mut d = vec![0u8; 16];
    let value = b"Words\nmore words";
    let mut items = Vec::new();
    items.extend_from_slice(&(value.len() as u32).to_le_bytes());
    items.extend_from_slice(&0u32.to_le_bytes());
    items.extend_from_slice(b"Lyrics\0");
    items.extend_from_slice(value);
    let mut foot = b"APETAGEX".to_vec();
    foot.extend_from_slice(&2000u32.to_le_bytes());
    foot.extend_from_slice(&(items.len() as u32 + 32).to_le_bytes());
    foot.extend_from_slice(&1u32.to_le_bytes());
    foot.extend_from_slice(&0u32.to_le_bytes());
    foot.extend_from_slice(&[0; 8]);
    d.extend(items);
    d.extend(foot);
    let t = read(&d);
    assert_eq!(t.lyrics.as_deref(), Some("Words\nmore words"));
}
