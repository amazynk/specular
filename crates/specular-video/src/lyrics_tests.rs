use super::*;

fn lrc(s: &str) -> Lyrics {
    parse_lrc(s, From::Sidecar).unwrap()
}

#[test]
fn lrc_lines_tags_and_offsets() {
    let l = lrc("\u{FEFF}[ar:Someone]\n[ti:A Song]\n[length: 03:20]\n\
         [00:12.00]First line\n[00:17.20][01:02.5]Chorus\n[00:21:10] Third \n\
         [1:00:00.00]An hour in\nno stamp here\n[00:30]\n");
    assert!(l.synced);
    let got: Vec<(f64, &str)> = l
        .lines
        .iter()
        .map(|x| (x.at.unwrap(), x.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (12.0, "First line"),
            (17.2, "Chorus"),
            (21.1, "Third"),
            (30.0, ""),
            (62.5, "Chorus"),
            (3600.0, "An hour in"),
        ]
    );
    // Before the first line: none; on a stamp: that line; between: the last started.
    assert_eq!(l.line_at(5.0), None);
    assert_eq!(l.line_at(12.0), Some(0));
    assert_eq!(l.line_at(20.0), Some(1));
    assert_eq!(l.line_at(100.0), Some(4));
    // A positive offset brings lines earlier (milliseconds).
    let o = lrc("[offset:+500]\n[00:10.00]a\n[00:00.20]b");
    assert_eq!(o.lines[0].at, Some(0.0));
    assert_eq!(o.lines[1].at, Some(9.5));
    let o = lrc("[offset:-250]\n[00:10.00]a");
    assert_eq!(o.lines[0].at, Some(10.25));
    // Three-digit fractions.
    assert_eq!(lrc("[00:01.250]x").lines[0].at, Some(1.25));
    // A tag-only file is not synced lyrics.
    assert_eq!(parse_lrc("[ar:x]\n[ti:y]\n", From::Tags), None);
}

#[test]
fn enhanced_lrc_words() {
    let l = lrc(
        "[00:10.00]<00:10.00> Hello <00:10.50> big <00:11.00> world <00:12.00>\n\
         [00:12.00]plain line",
    );
    let line = &l.lines[0];
    assert_eq!(line.text, "Hello big world");
    let words: Vec<(f64, &str)> = line.words.iter().map(|w| (w.at, w.text.as_str())).collect();
    assert_eq!(words, [(10.0, "Hello "), (10.5, "big "), (11.0, "world")]);
    assert!(l.lines[1].words.is_empty());
    // How much is sung: none before, all after, part way through a word.
    assert_eq!(l.sung(0, 9.0), Some(0.0));
    assert!(l.sung(0, 11.99).unwrap() > 0.99);
    assert!((l.sung(0, 12.0).unwrap() - 1.0).abs() < 1e-6);
    let half = l.sung(0, 10.25).unwrap();
    // "Hello " is 6 of 15 characters; half of it sung.
    assert!((half - 3.0 / 15.0).abs() < 1e-6, "{half}");
    assert_eq!(l.sung(1, 13.0), None);
    // A word stamp with something not a stamp in angle brackets stays text.
    let x = lrc("[00:01.00]a <b> c");
    assert_eq!(x.lines[0].text, "a <b> c");
}

#[test]
fn plain_lyrics() {
    let l = Lyrics::from_text("\n\nVerse one\r\nline two\n\nVerse two\n\n", From::Tags).unwrap();
    assert!(!l.synced);
    let t: Vec<&str> = l.lines.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(t, ["Verse one", "line two", "", "Verse two"]);
    assert_eq!(l.line_at(10.0), None);
    assert_eq!(l.text(), "Verse one\nline two\n\nVerse two");
    assert_eq!(Lyrics::from_text("  \n ", From::Tags), None);
    // LRC text in a tag is synced.
    assert!(Lyrics::from_text("[00:01.00]x", From::Tags).unwrap().synced);
}

fn sylt(enc: u8, entries: &[(&str, u32)]) -> Vec<u8> {
    let mut f = vec![enc, b'e', b'n', b'g', 2, 1];
    f.push(0); // empty description (ISO-8859-1 / UTF-8 terminator)
    for (text, ms) in entries {
        f.extend_from_slice(text.as_bytes());
        f.push(0);
        f.extend_from_slice(&ms.to_be_bytes());
    }
    f
}

#[test]
fn sylt_lines_and_syllables() {
    // An entry a line.
    let l = parse_sylt(&sylt(3, &[("One", 1000), ("Two", 2500)])).unwrap();
    assert!(l.synced);
    assert_eq!(l.lines.len(), 2);
    assert_eq!(l.lines[1].at, Some(2.5));
    assert!(l.lines[0].words.is_empty());
    // Syllables, lines started by a line break.
    let l = parse_sylt(&sylt(
        3,
        &[
            ("Hel", 1000),
            ("lo ", 1200),
            ("you", 1500),
            ("\nNext", 3000),
        ],
    ))
    .unwrap();
    assert_eq!(l.lines.len(), 2);
    assert_eq!(l.lines[0].text, "Hello you");
    assert_eq!(l.lines[0].words.len(), 3);
    assert_eq!(l.lines[1].text, "Next");
    // UTF-16 with byte-order marks.
    let mut f = vec![1, b'e', b'n', b'g', 2, 1, 0xFF, 0xFE, 0, 0];
    f.extend_from_slice(&[0xFF, 0xFE]);
    f.extend("Été".encode_utf16().flat_map(u16::to_le_bytes));
    f.extend_from_slice(&[0, 0]);
    f.extend_from_slice(&500u32.to_be_bytes());
    let l = parse_sylt(&f).unwrap();
    assert_eq!(l.lines[0].text, "Été");
    assert_eq!(l.lines[0].at, Some(0.5));
    // MPEG-frame times are not read.
    let mut f = sylt(3, &[("x", 1)]);
    f[4] = 1;
    assert_eq!(parse_sylt(&f), None);
    // Cut short: what is whole.
    let f = sylt(3, &[("a", 100), ("b", 200)]);
    assert_eq!(parse_sylt(&f[..f.len() - 2]).unwrap().lines.len(), 1);
}

#[test]
fn text_encodings() {
    assert_eq!(decode_text(b"\xEF\xBB\xBFabc"), "abc");
    assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
    assert_eq!(decode_text(&[0xFE, 0xFF, 0, b'h', 0, b'i']), "hi");
    assert_eq!(decode_text("Gr\u{fc}n".as_bytes()), "Grün");
    // Not UTF-8: Latin-1.
    assert_eq!(decode_text(b"Gr\xFCn"), "Grün");
}

#[test]
fn sidecar_beside_the_sound() {
    let dir = std::env::temp_dir().join("specular-lyrics-sidecar");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let song = dir.join("song.flac");
    std::fs::write(&song, b"").unwrap();
    assert_eq!(read_sidecar(&song), None);
    std::fs::write(dir.join("song.lrc"), "[00:01.00]Hi\n").unwrap();
    let l = read_sidecar(&song).unwrap();
    assert_eq!(l.from, From::Sidecar);
    assert_eq!(l.lines[0].text, "Hi");
    let _ = std::fs::remove_dir_all(&dir);
}
