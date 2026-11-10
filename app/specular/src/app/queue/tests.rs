use std::collections::HashSet;
use std::path::{Path, PathBuf};

use specular_settings::{PlayRepeat, SortOrder};

use super::*;

/// A folder under the workspace's `target/test-tmp`, emptied.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target/test-tmp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A mono 16-bit PCM WAV of `secs` seconds of a sine at `hz`, 8 kHz.
fn wav(secs: f64, hz: f64) -> Vec<u8> {
    let rate = 8000u32;
    let n = (secs * f64::from(rate)) as u32;
    let mut pcm = Vec::with_capacity(n as usize * 2);
    for i in 0..n {
        let v = (f64::from(i) / f64::from(rate) * hz * std::f64::consts::TAU).sin() * 8000.0;
        pcm.extend_from_slice(&(v as i16).to_le_bytes());
    }
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(&pcm);
    w
}

/// Three sound files and a note in a folder: the sound files of the folder as the viewer
/// lists it.
fn three_wavs(name: &str) -> (PathBuf, Vec<PathBuf>) {
    let dir = scratch(name);
    for (file, secs, hz) in [
        ("10 last.wav", 1.5, 660.0),
        ("02 second.wav", 1.0, 550.0),
        ("01 first.wav", 0.5, 440.0),
    ] {
        std::fs::write(dir.join(file), wav(secs, hz)).unwrap();
    }
    std::fs::write(dir.join("notes.txt"), "not a sound").unwrap();
    specular_video::install();
    let registry = specular_decode::Registry::new();
    let opts = crate::loader::ListOptions {
        order: SortOrder::Natural,
        descending: false,
        recursive: false,
    };
    let listed = crate::loader::list_folder(&dir, &registry, opts);
    // The viewer lists the note (text files show in the gallery); a play queue never takes
    // it.
    assert!(listed.iter().any(|p| p.ends_with("notes.txt")));
    let sounds = candidates(&listed, Kind::Sound);
    assert!(!sounds.iter().any(|p| p.ends_with("notes.txt")));
    assert_eq!(Kind::of(Path::new("notes.txt")), None);
    (dir, sounds)
}

/// A folder of three generated WAVs: the viewer lists them in natural order, reads each
/// one's length from its header (as the queue panel does, off the main thread), and at the
/// end of each the next plays, until the last ends (repeat off) or the first comes again
/// (repeat all); shuffled, each plays once.
#[test]
fn folder_of_three_wavs_plays_through() {
    let (dir, listed) = three_wavs("queue-three-wavs");
    let names: Vec<_> = listed
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["01 first.wav", "02 second.wav", "10 last.wav"]);
    let registry = specular_decode::Registry::new();
    for (p, secs) in listed.iter().zip([0.5, 1.0, 1.5]) {
        let s = registry.open_path(p).unwrap();
        assert!(
            s.video().is_some_and(|v| v.sound.is_some()),
            "{} opens as a sound file",
            p.display()
        );
        let (_, title, duration) = read_info(&registry, p);
        assert_eq!(title, None);
        let d = duration.expect("a length");
        assert!((d - secs).abs() < 0.02, "{}: {d}", p.display());
    }
    let list = candidates(&listed, Kind::Sound);
    assert_eq!(list, listed);
    assert!(candidates(&listed, Kind::Video).is_empty());

    // Repeat off: first → second → last, then nothing.
    let mut q = folder_queue(&list, PlayRepeat::Off, false, 1);
    q.play(0);
    let mut played = vec![0];
    while let Some(i) = q.advance(true) {
        played.push(i);
        assert!(played.len() < 10);
    }
    assert_eq!(played, [0, 1, 2]);
    // Repeat all: the first again after the last.
    let mut q = folder_queue(&list, PlayRepeat::All, false, 1);
    q.play(2);
    assert_eq!(q.next_item(true), Some(0));
    // Repeat one: the same file.
    let mut q = folder_queue(&list, PlayRepeat::One, false, 1);
    q.play(1);
    assert_eq!(q.next_item(true), Some(1));
    assert_eq!(q.next_item(false), Some(2));
    // Shuffled from the second: the other two, once each, then nothing.
    for seed in 1..20 {
        let mut q = folder_queue(&list, PlayRepeat::Off, true, seed);
        q.play(1);
        let mut rest = Vec::new();
        while let Some(i) = q.advance(true) {
            rest.push(i);
            assert!(rest.len() < 10);
        }
        rest.sort_unstable();
        assert_eq!(rest, [0, 2], "seed {seed}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Folders give their sound files and videos (not subfolders'), playlists their items;
/// images and other files nothing.
#[test]
fn what_goes_into_the_queue() {
    let (dir, listed) = three_wavs("queue-expand");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/deep.wav"), wav(0.2, 300.0)).unwrap();
    std::fs::write(dir.join("clip.mp4"), b"not really").unwrap();
    std::fs::write(dir.join("photo.jpg"), b"not really").unwrap();
    std::fs::write(
        dir.join("list.m3u"),
        "#EXTM3U\n#EXTINF:2,Two\n02 second.wav\nhttp://radio.example/live\n",
    )
    .unwrap();
    let names = |v: &[Entry]| v.iter().map(Entry::name).collect::<Vec<_>>();
    let from_dir = expand(std::slice::from_ref(&dir));
    assert_eq!(
        names(&from_dir),
        ["01 first.wav", "02 second.wav", "10 last.wav", "clip.mp4"]
    );
    let picked = expand(&[
        dir.join("photo.jpg"),
        listed[2].clone(),
        dir.join("list.m3u"),
    ]);
    assert_eq!(
        names(&picked),
        ["10 last.wav", "02 second.wav", "http://radio.example/live"]
    );
    assert_eq!(picked[1].title.as_deref(), Some("Two"));
    assert!(picked[2].playable(), "a web address plays as a stream");
    assert!(expand(&[dir.join("photo.jpg")]).is_empty());
    assert_eq!(Kind::of(Path::new("a.FLAC")), Some(Kind::Sound));
    assert_eq!(Kind::of(Path::new("a.mkv")), Some(Kind::Video));
    assert_eq!(Kind::of(Path::new("a.png")), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The queue saved as M3U8 and XSPF opens again as the same items.
#[test]
fn saved_queue_opens_again() {
    let (dir, listed) = three_wavs("queue-save");
    let entries: Vec<Entry> = listed.iter().cloned().map(Entry::file).collect();
    for name in ["q.m3u8", "q.xspf"] {
        let dest = dir.join("lists").join(name);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let text = if name.ends_with("xspf") {
            specular_playlist::write_xspf(Some("Test"), &entries, &dest)
        } else {
            specular_playlist::write_m3u8(Some("Test"), &entries, &dest)
        };
        std::fs::write(&dest, text).unwrap();
        let back = expand(std::slice::from_ref(&dest));
        let paths: Vec<_> = back
            .iter()
            .map(|e| e.location.path().unwrap().to_path_buf())
            .collect();
        assert_eq!(paths, listed, "{name}");
        let set: HashSet<_> = paths.iter().collect();
        assert_eq!(set.len(), 3);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
