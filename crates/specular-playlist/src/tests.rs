use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::*;

fn q(n: usize) -> Queue {
    let mut q = Queue::new(42);
    q.append(
        (0..n)
            .map(|i| Entry::file(format!("/m/{i}.flac")))
            .collect(),
    );
    q
}

fn url(u: &str) -> Entry {
    Entry {
        location: Location::Url(u.into()),
        title: None,
        duration: None,
    }
}

/// Plays from the start until the end (at most `limit` items).
fn play_through(q: &mut Queue, limit: usize) -> Vec<usize> {
    let mut seen = Vec::new();
    while let Some(i) = q.advance(true) {
        seen.push(i);
        if seen.len() >= limit {
            break;
        }
    }
    seen
}

#[test]
fn in_order_to_the_end_then_stops() {
    let mut q = q(3);
    assert_eq!(q.next_item(true), Some(0));
    assert_eq!(play_through(&mut q, 10), vec![0, 1, 2]);
    assert_eq!(q.current(), Some(2));
    assert_eq!(q.next_item(true), None);
}

#[test]
fn next_item_has_no_side_effects_and_matches_advance() {
    for shuffle in [false, true] {
        for repeat in [Repeat::Off, Repeat::All, Repeat::One] {
            let mut q = q(7);
            q.set_shuffle(shuffle);
            q.set_repeat(repeat);
            for step in 0..30 {
                let auto = step % 3 != 0;
                let before = format!("{q:?}");
                let n = q.next_item(auto);
                assert_eq!(before, format!("{q:?}"), "next_item changed the queue");
                assert_eq!(q.advance(auto), n, "{shuffle} {repeat:?} step {step}");
                if n.is_none() {
                    break;
                }
            }
        }
    }
}

#[test]
fn repeat_all_wraps_and_repeat_one_stays() {
    let mut q = q(3);
    q.set_repeat(Repeat::All);
    assert_eq!(play_through(&mut q, 7), vec![0, 1, 2, 0, 1, 2, 0]);
    let mut q = super::tests::q(3);
    q.set_repeat(Repeat::One);
    q.play(1);
    assert_eq!(q.next_item(true), Some(1));
    assert_eq!(q.advance(true), Some(1));
    // Next by hand moves on, and wraps.
    assert_eq!(q.advance(false), Some(2));
    assert_eq!(q.advance(false), Some(0));
    assert_eq!(Repeat::Off.cycled(), Repeat::All);
    assert_eq!(Repeat::All.cycled(), Repeat::One);
    assert_eq!(Repeat::One.cycled(), Repeat::Off);
}

#[test]
fn previous_goes_up_and_wraps_only_with_repeat() {
    let mut q = q(3);
    q.play(0);
    assert_eq!(q.prev_item(), None);
    q.set_repeat(Repeat::All);
    assert_eq!(q.prev_item(), Some(2));
    q.set_repeat(Repeat::Off);
    q.play(2);
    assert_eq!(q.go_back(), Some(1));
    assert_eq!(q.go_back(), Some(0));
}

#[test]
fn shuffle_plays_each_once_per_round() {
    for seed in 1..40u64 {
        let mut q = Queue::new(seed);
        q.append((0..9).map(|i| Entry::file(format!("/m/{i}.mp3"))).collect());
        q.set_shuffle(true);
        let round = play_through(&mut q, 100);
        assert_eq!(round.len(), 9, "seed {seed}: {round:?}");
        let set: HashSet<_> = round.iter().copied().collect();
        assert_eq!(set.len(), 9, "seed {seed}: a repeat in {round:?}");
        assert_eq!(q.next_item(true), None);
        // Repeat all: another full round, never the same item twice in a row.
        q.set_repeat(Repeat::All);
        let mut more = vec![*round.last().unwrap()];
        more.extend(play_through(&mut q, 18));
        for w in more.windows(2) {
            assert_ne!(w[0], w[1], "seed {seed}");
        }
        for chunk in more[1..].chunks(9) {
            let set: HashSet<_> = chunk.iter().copied().collect();
            assert_eq!(set.len(), chunk.len(), "seed {seed}: {more:?}");
        }
    }
}

#[test]
fn shuffle_is_not_list_order() {
    let mut differs = 0;
    for seed in 1..20u64 {
        let mut q = Queue::new(seed);
        q.append((0..8).map(|i| Entry::file(format!("/m/{i}.mp3"))).collect());
        q.set_shuffle(true);
        if play_through(&mut q, 100) != (0..8).collect::<Vec<_>>() {
            differs += 1;
        }
    }
    assert!(differs >= 18, "{differs}");
}

#[test]
fn shuffle_history_lets_previous_go_back() {
    let mut q = q(6);
    q.set_shuffle(true);
    let a = q.advance(true).unwrap();
    let b = q.advance(true).unwrap();
    let c = q.advance(true).unwrap();
    assert_eq!(q.prev_item(), Some(b));
    assert_eq!(q.go_back(), Some(b));
    assert_eq!(q.go_back(), Some(a));
    assert_eq!(q.go_back(), None);
    // Forward again replays what was left, then the rest of the round.
    assert_eq!(q.advance(false), Some(b));
    assert_eq!(q.advance(false), Some(c));
    let rest = play_through(&mut q, 10);
    assert_eq!(rest.len(), 3);
    let all: HashSet<_> = [a, b, c].into_iter().chain(rest).collect();
    assert_eq!(all.len(), 6);
}

#[test]
fn web_addresses_play_and_other_addresses_are_skipped() {
    let mut q = Queue::new(7);
    q.append(vec![
        url("http://radio.example/stream"),
        Entry::file("/m/a.mp3"),
        url("rtsp://x.example/b"),
        url("HTTPS://x.example/b.mp3"),
        Entry::file("/m/c.mp3"),
    ]);
    assert_eq!(play_through(&mut q, 10), vec![0, 1, 3, 4]);
    q.clear();
    q.append(vec![url("rtsp://a"), url("mms://b")]);
    assert_eq!(q.next_item(true), None);
    q.set_shuffle(true);
    assert_eq!(q.next_item(true), None);
    q.set_repeat(Repeat::All);
    assert_eq!(q.next_item(false), None);
}

#[test]
fn play_next_goes_after_the_current_item() {
    let mut q = q(4);
    q.play(1);
    q.insert_next(vec![Entry::file("/n/x.mp3"), Entry::file("/n/y.mp3")]);
    assert_eq!(q.current(), Some(1));
    assert_eq!(q.len(), 6);
    assert_eq!(q.get(2).unwrap().name(), "x.mp3");
    assert_eq!(play_through(&mut q, 10), vec![2, 3, 4, 5]);
    // Shuffled: the new ones still come next, in order.
    let mut q = super::tests::q(5);
    q.set_shuffle(true);
    q.advance(true);
    let r = q.insert_next(vec![Entry::file("/n/x.mp3"), Entry::file("/n/y.mp3")]);
    assert_eq!(q.advance(true), Some(r.start));
    assert_eq!(q.advance(true), Some(r.start + 1));
    let rest = play_through(&mut q, 10);
    assert_eq!(rest.len(), 4, "{rest:?}");
}

#[test]
fn remove_and_move_keep_the_current_item_and_order() {
    let mut q = q(5);
    q.play(2);
    q.remove(0);
    assert_eq!(q.current(), Some(1));
    assert_eq!(q.current_entry().unwrap().name(), "2.flac");
    // Removing the current item: the one after it plays next.
    q.remove(1);
    assert_eq!(q.current(), None);
    assert_eq!(q.next_item(true), Some(1));
    assert_eq!(q.get(1).unwrap().name(), "3.flac");
    // Drag: the last to the top.
    let mut q = super::tests::q(4);
    q.play(1);
    q.move_item(3, 0);
    let names: Vec<_> = q.items().iter().map(Entry::name).collect();
    assert_eq!(names, ["3.flac", "0.flac", "1.flac", "2.flac"]);
    assert_eq!(q.current(), Some(2));
    assert_eq!(q.next_item(true), Some(3));
    q.move_item(2, 3);
    assert_eq!(q.current(), Some(3));
    assert_eq!(q.next_item(true), None);
    // Shuffled: a moved item keeps its place in the round.
    let mut q = super::tests::q(6);
    q.set_shuffle(true);
    let first = q.next_item(true).unwrap();
    q.move_item(first, 5);
    assert_eq!(q.next_item(true), Some(5));
    let round = play_through(&mut q, 20);
    assert_eq!(round.iter().copied().collect::<HashSet<_>>().len(), 6);
}

#[test]
fn detach_and_find() {
    let mut q = q(3);
    q.play(1);
    q.detach();
    assert_eq!(q.current(), None);
    assert_eq!(q.next_item(true), Some(2));
    assert_eq!(q.find(Path::new("/m/0.flac")), Some(0));
    assert_eq!(q.find(Path::new("/m/9.flac")), None);
    q.clear();
    assert!(q.is_empty());
    assert_eq!(q.next_item(true), None);
}

// ----- Playlists ------------------------------------------------------------------------

fn base() -> PathBuf {
    PathBuf::from("/music/lists")
}

fn files(p: &Playlist) -> Vec<String> {
    p.entries
        .iter()
        .map(|e| match &e.location {
            Location::File(f) => f.to_string_lossy().into_owned(),
            Location::Url(u) => format!("url:{u}"),
        })
        .collect()
}

#[test]
fn m3u_extended_with_relative_paths_and_urls() {
    let text = "#EXTM3U\r\n#PLAYLIST:Road trip\r\n#EXTINF:215,Artist – Song, with comma\r\n\
                ../album/01 Song.flac\r\n\r\n# a comment\r\n\
                #EXTINF:-1 tvg-id=\"x,y\" logo=\"a\",Radio\r\nhttp://radio.example/live\r\n\
                /abs/02.mp3\r\nfile:///abs/My%20Song.ogg\r\nsub\\dir\\03.wav\r\n";
    let p = read(text.as_bytes(), Kind::M3u, &base()).unwrap();
    assert_eq!(p.title.as_deref(), Some("Road trip"));
    assert_eq!(
        files(&p),
        [
            "/music/album/01 Song.flac",
            "url:http://radio.example/live",
            "/abs/02.mp3",
            "/abs/My Song.ogg",
            "/music/lists/sub/dir/03.wav",
        ]
    );
    assert_eq!(p.entries[0].duration, Some(215.0));
    assert_eq!(
        p.entries[0].title.as_deref(),
        Some("Artist – Song, with comma")
    );
    assert_eq!(p.entries[1].duration, None);
    assert_eq!(p.entries[1].title.as_deref(), Some("Radio"));
    assert_eq!(p.entries[2].title, None);
    assert!(p.entries[1].playable(), "a web address plays as a stream");
}

#[test]
fn m3u_plain_latin1_and_utf8_bom() {
    // "Café.mp3" in Latin-1 (é = 0xE9), as an old Windows player wrote it.
    let latin1 = b"Caf\xe9.mp3\nNa\xefve.mp3\n";
    let p = read(latin1, Kind::M3u, &base()).unwrap();
    assert_eq!(
        files(&p),
        ["/music/lists/Café.mp3", "/music/lists/Naïve.mp3"]
    );
    let mut bom = b"\xEF\xBB\xBF#EXTM3U\n#EXTINF:3,\xD7\xA9\xD7\x99\xD7\xA8\n".to_vec();
    bom.extend_from_slice("שיר.flac\n".as_bytes());
    let p = read(&bom, Kind::M3u, &base()).unwrap();
    assert_eq!(files(&p), ["/music/lists/שיר.flac"]);
    assert_eq!(p.entries[0].title.as_deref(), Some("שיר"));
    // UTF-16 with a byte-order mark.
    let mut u16 = vec![0xFF, 0xFE];
    for c in "a.mp3\n".encode_utf16() {
        u16.extend_from_slice(&c.to_le_bytes());
    }
    assert_eq!(
        files(&read(&u16, Kind::M3u, &base()).unwrap()),
        ["/music/lists/a.mp3"]
    );
}

#[test]
fn pls_in_number_order() {
    let text = "[playlist]\nNumberOfEntries=3\nFile2=b.mp3\nTitle2=Bee\nLength2=61\n\
                File1=http://stream.example:8000/\nTitle1=Stream\nLength1=-1\n\
                file10=../c.ogg\nVersion=2\n[other]\nFile3=nope.mp3\n";
    let p = read(text.as_bytes(), Kind::Pls, &base()).unwrap();
    assert_eq!(
        files(&p),
        [
            "url:http://stream.example:8000/",
            "/music/lists/b.mp3",
            "/music/c.ogg"
        ]
    );
    assert_eq!(p.entries[1].title.as_deref(), Some("Bee"));
    assert_eq!(p.entries[1].duration, Some(61.0));
    assert_eq!(p.entries[0].duration, None);
}

#[test]
fn xspf_tracks() {
    let text = r#"<?xml version="1.0" encoding="UTF-8"?>
<playlist version="1" xmlns="http://xspf.org/ns/0/">
  <title>Mix &amp; match</title>
  <trackList>
    <track><location>file:///home/ana/Music/a%20b.flac</location><title>A B</title>
      <creator>Band</creator><duration>183500</duration></track>
    <track><location>rel/c%C3%A9.mp3</location></track>
    <track><location>https://example.org/x.mp3</location><title>Web</title></track>
    <track><title>no location</title></track>
  </trackList>
</playlist>"#;
    let p = read(text.as_bytes(), Kind::Xspf, &base()).unwrap();
    assert_eq!(p.title.as_deref(), Some("Mix & match"));
    assert_eq!(
        files(&p),
        [
            "/home/ana/Music/a b.flac",
            "/music/lists/rel/cé.mp3",
            "url:https://example.org/x.mp3"
        ]
    );
    assert_eq!(p.entries[0].title.as_deref(), Some("Band – A B"));
    assert_eq!(p.entries[0].duration, Some(183.5));
    assert!(matches!(
        read(b"<html/>", Kind::Xspf, &base()),
        Err(Error::NotPlaylist)
    ));
    assert!(matches!(
        read(b"<playlist", Kind::Xspf, &base()),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn kinds_by_name_and_content() {
    assert_eq!(Kind::of_path(Path::new("a.M3U8")), Some(Kind::M3u));
    assert_eq!(Kind::of_path(Path::new("a.pls")), Some(Kind::Pls));
    assert_eq!(Kind::of_path(Path::new("a.xspf")), Some(Kind::Xspf));
    assert_eq!(Kind::of_path(Path::new("a.mp3")), None);
    assert_eq!(Kind::probe(b"\xEF\xBB\xBF#EXTM3U\n"), Some(Kind::M3u));
    assert_eq!(Kind::probe(b"[Playlist]\r\n"), Some(Kind::Pls));
    assert_eq!(
        Kind::probe(b"<?xml version=\"1.0\"?>\n<playlist xmlns=\"http://xspf.org/ns/0/\">"),
        Some(Kind::Xspf)
    );
    assert_eq!(Kind::probe(b"a.mp3\n"), None);
}

#[test]
fn saved_playlists_read_back_the_same() {
    let dir = PathBuf::from("/home/ana/Lists");
    let dest = dir.join("q.m3u8");
    let entries = vec![
        Entry {
            location: Location::File(dir.join("sub/01 Ünïcode & <x>.flac")),
            title: Some("Song, one".into()),
            duration: Some(201.4),
        },
        Entry {
            location: Location::File(PathBuf::from("/srv/music/02 a%b.mp3")),
            title: None,
            duration: None,
        },
        url("http://radio.example/live"),
    ];
    let m3u = write_m3u8(Some("Evening"), &entries, &dest);
    assert!(m3u.starts_with("#EXTM3U\n#PLAYLIST:Evening\n#EXTINF:201,Song, one\nsub/01"));
    let back = read(m3u.as_bytes(), Kind::M3u, &dir).unwrap();
    assert_eq!(back.title.as_deref(), Some("Evening"));
    let locs: Vec<_> = back.entries.iter().map(|e| e.location.clone()).collect();
    let want: Vec<_> = entries.iter().map(|e| e.location.clone()).collect();
    assert_eq!(locs, want);
    assert_eq!(back.entries[0].duration, Some(201.0));
    // The second has no title: its file name is written.
    assert_eq!(back.entries[1].title.as_deref(), Some("02 a%b.mp3"));

    let xspf = write_xspf(Some("Evening"), &entries, &dir.join("q.xspf"));
    assert!(xspf.contains("<location>sub/01%20%C3%9Cn%C3%AFcode%20%26%20%3Cx%3E.flac</location>"));
    assert!(xspf.contains("<location>file:///srv/music/02%20a%25b.mp3</location>"));
    let back = read(xspf.as_bytes(), Kind::Xspf, &dir).unwrap();
    let locs: Vec<_> = back.entries.iter().map(|e| e.location.clone()).collect();
    assert_eq!(locs, want);
    assert_eq!(back.entries[0].title.as_deref(), Some("Song, one"));
    assert_eq!(back.entries[0].duration, Some(201.4));
}

#[test]
fn file_uris() {
    assert_eq!(
        path_from_file_uri("file://localhost/a/b%20c"),
        Some(PathBuf::from("/a/b c"))
    );
    assert_eq!(path_from_file_uri("file://server/share/x"), None);
    assert_eq!(path_from_file_uri("http://x/y"), None);
    assert_eq!(
        path_from_file_uri("file:///C:/Music/a.mp3"),
        Some(PathBuf::from("C:/Music/a.mp3"))
    );
    assert_eq!(file_uri(Path::new("/a/b c#.mp3")), "file:///a/b%20c%23.mp3");
}

/// Files written by the test and read back from disk (the kind from the name, else the
/// content).
#[test]
fn read_file_from_disk() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target/test-tmp/specular-playlist-read-file");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("list.txt");
    std::fs::write(&p, "#EXTM3U\na.mp3\n").unwrap();
    let pl = read_file(&p).unwrap();
    assert_eq!(pl.entries[0].location, Location::File(dir.join("a.mp3")));
    let p = dir.join("plain.txt");
    std::fs::write(&p, "a.mp3\n").unwrap();
    assert!(matches!(
        read_file(&p),
        Err(ReadFileError::Playlist(Error::NotPlaylist))
    ));
    assert!(matches!(
        read_file(&dir.join("missing.m3u")),
        Err(ReadFileError::Io(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
