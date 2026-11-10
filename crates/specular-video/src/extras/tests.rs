use super::*;
use crate::mkv::build::{el, header, uel};
use crate::mkv::coding::{
    CONTENT_COMP_ALGO, CONTENT_COMP_SETTINGS, CONTENT_COMPRESSION, CONTENT_ENCODING,
};
use crate::mp4::build::{bx, full};

fn be32(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn mkv_track(n: u64, kind: u64, codec: &str, lang: &str, extra: &[u8]) -> Vec<u8> {
    let mut b = uel(TRACK_NUMBER, n);
    b.extend(uel(TRACK_TYPE, kind));
    b.extend(el(CODEC_ID, codec.as_bytes()));
    if !lang.is_empty() {
        b.extend(el(LANGUAGE, lang.as_bytes()));
    }
    b.extend_from_slice(extra);
    el(TRACK_ENTRY, &b)
}

/// A block group: track `n`, time `rel` ms after the cluster's, lasting `dur` ms.
fn group(n: u8, rel: i16, dur: u64, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![0x80 | n];
    b.extend_from_slice(&rel.to_be_bytes());
    b.push(0);
    b.extend_from_slice(payload);
    let mut g = el(0xA1, &b);
    g.extend(uel(0x9B, dur));
    el(0xA0, &g)
}

fn chapter(start_ms: u64, names: &[(&str, &str)], hidden: bool) -> Vec<u8> {
    let mut a = uel(CHAPTER_TIME_START, start_ms * 1_000_000);
    for (lang, name) in names {
        let mut d = el(CHAP_STRING, name.as_bytes());
        d.extend(el(CHAP_LANGUAGE, lang.as_bytes()));
        a.extend(el(CHAPTER_DISPLAY, &d));
    }
    if hidden {
        a.extend(uel(CHAPTER_FLAG_HIDDEN, 1));
    }
    el(CHAPTER_ATOM, &a)
}

fn mkv_file() -> Vec<u8> {
    let mut tracks = mkv_track(1, 2, "A_AAC", "eng", &[]);
    let mut dub = el(NAME, "Director's commentary".as_bytes());
    dub.extend(uel(FLAG_DEFAULT, 0));
    dub.extend(el(AUDIO, &uel(CHANNELS, 6)));
    tracks.extend(mkv_track(2, 2, "A_AC3", "heb", &dub));
    tracks.extend(mkv_track(3, 0x11, "S_TEXT/UTF8", "heb", &[]));
    // Header stripping: "<i>" is stripped from every frame.
    let mut comp = uel(CONTENT_COMP_ALGO, 3);
    comp.extend(el(CONTENT_COMP_SETTINGS, b"<i>"));
    let enc = el(
        CONTENT_ENCODINGS,
        &el(CONTENT_ENCODING, &el(CONTENT_COMPRESSION, &comp)),
    );
    let mut forced = enc;
    forced.extend(uel(FLAG_FORCED, 1));
    forced.extend(el(LANGUAGE_BCP47, b"en-US"));
    tracks.extend(mkv_track(4, 0x11, "S_TEXT/UTF8", "eng", &forced));
    let info = el(INFO, &uel(TIMESTAMP_SCALE, 1_000_000));
    let mut chapters = chapter(0, &[("eng", "Opening"), ("heb", "פתיחה")], false);
    chapters.extend(chapter(500, &[("eng", "Hidden")], true));
    chapters.extend(chapter(60_000, &[("eng", "The end")], false));
    let chapters = el(CHAPTERS, &el(EDITION_ENTRY, &chapters));
    let mut cluster = uel(0xE7, 1000);
    cluster.extend(group(3, 0, 1500, "שלום".as_bytes()));
    cluster.extend(group(4, 200, 800, b"Hi</i>"));
    cluster.extend(group(3, 2000, 1000, b"<b>two</b>"));
    let mut seg = info;
    seg.extend(el(TRACKS, &tracks));
    seg.extend(chapters);
    seg.extend(el(CLUSTER, &cluster));
    let mut f = header("matroska");
    f.extend(el(SEGMENT, &seg));
    f
}

#[test]
fn matroska_tracks_subtitles_and_chapters() {
    let x = read(&mkv_file());
    assert_eq!(x.audio.len(), 2);
    assert_eq!(x.audio[0].lang.as_deref(), Some("eng"));
    assert!(x.audio[0].default);
    assert_eq!(x.audio[1].name.as_deref(), Some("Director's commentary"));
    assert_eq!((x.audio[1].channels, x.audio[1].default), (6, false));
    assert_eq!(x.subtitles.len(), 2);
    let he = &x.subtitles[0];
    assert_eq!((he.info.id, he.info.codec.as_str()), (3, "S_TEXT/UTF8"));
    assert_eq!(he.packets.len(), 2);
    assert_eq!(he.packets[0].start_us, 1_000_000);
    assert_eq!(he.packets[0].dur_us, Some(1_500_000));
    assert_eq!(he.packets[0].data, "שלום".as_bytes());
    assert_eq!(he.packets[1].start_us, 3_000_000);
    let en = &x.subtitles[1];
    assert!(en.info.forced);
    assert_eq!(en.info.lang.as_deref(), Some("en-US"));
    assert_eq!(en.packets[0].data, b"<i>Hi</i>", "stripped header put back");
    assert_eq!(x.chapters.len(), 2, "the hidden chapter is left out");
    assert_eq!(x.chapters[0].name("he"), Some("פתיחה"));
    assert_eq!(x.chapters[0].name("fr"), Some("Opening"));
    assert_eq!(x.chapters[1].start_us, 60_000_000);
}

#[test]
fn matroska_sound_choice() {
    let f = mkv_file();
    let m = crate::mkv::parse(&f, false).unwrap();
    assert!(matches!(m.audio.unwrap().codec, mp4::AudioCodec::Aac(_)));
    let m = crate::mkv::parse_with(&f, false, Some(2)).unwrap();
    assert!(matches!(m.audio.unwrap().codec, mp4::AudioCodec::Ac3));
    // An id that is not a sound track: the default.
    let m = crate::mkv::parse_with(&f, false, Some(3)).unwrap();
    assert!(matches!(m.audio.unwrap().codec, mp4::AudioCodec::Aac(_)));
}

/// An MP4 track: handler, sample entry (type and body), language, timescale 1000, samples
/// each `dur` ms long at `offsets` in the file.
#[allow(clippy::too_many_arguments)]
fn mp4_trak(
    id: u32,
    handler: &[u8; 4],
    entry: &[u8; 4],
    entry_body: &[u8],
    lang: &str,
    sizes: &[u32],
    offset: u32,
    dur: u32,
    tref_chap: Option<u32>,
) -> Vec<u8> {
    let mut tkhd = be32(&[0, 0, id, 0, 0]);
    tkhd.extend(vec![0u8; 60]);
    let l = lang.as_bytes();
    let packed = ((u16::from(l[0]) - 0x60) << 10)
        | ((u16::from(l[1]) - 0x60) << 5)
        | (u16::from(l[2]) - 0x60);
    let mut mdhd = be32(&[0, 0, 1000, dur * sizes.len() as u32]);
    mdhd.extend(packed.to_be_bytes());
    mdhd.extend([0, 0]);
    let mut hdlr = be32(&[0]);
    hdlr.extend_from_slice(handler);
    hdlr.extend(vec![0u8; 13]);
    let mut stsd = be32(&[1]);
    stsd.extend(bx(entry, entry_body));
    let mut stbl = full(b"stsd", 0, &stsd);
    stbl.extend(full(b"stts", 0, &be32(&[1, sizes.len() as u32, dur])));
    let mut stsz = be32(&[0, sizes.len() as u32]);
    stsz.extend(be32(sizes));
    stbl.extend(full(b"stsz", 0, &stsz));
    stbl.extend(full(b"stsc", 0, &be32(&[1, 1, sizes.len() as u32, 1])));
    stbl.extend(full(b"stco", 0, &be32(&[1, offset])));
    let mut mdia = full(b"mdhd", 0, &mdhd);
    mdia.extend(full(b"hdlr", 0, &hdlr));
    mdia.extend(bx(b"minf", &bx(b"stbl", &stbl)));
    let mut trak = full(b"tkhd", 0, &tkhd);
    // Enabled.
    trak[11] = 1;
    if let Some(c) = tref_chap {
        trak.extend(bx(b"tref", &bx(b"chap", &c.to_be_bytes())));
    }
    trak.extend(bx(b"mdia", &mdia));
    bx(b"trak", &trak)
}

fn text_sample(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u16).to_be_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}

#[test]
fn mp4_tracks_subtitles_and_chapters() {
    let subs = [text_sample("Hello"), text_sample("")];
    let chaps = [text_sample("Intro"), text_sample("Middle")];
    let mut f = bx(b"ftyp", b"isom\0\0\0\0isom");
    let data: Vec<u8> = subs.concat().into_iter().chain(chaps.concat()).collect();
    let at = f.len() as u32 + 8;
    f.extend(bx(b"mdat", &data));
    let sizes = |v: &[Vec<u8>]| v.iter().map(|s| s.len() as u32).collect::<Vec<_>>();
    let mut moov = full(b"mvhd", 0, &{
        let mut m = be32(&[0, 0, 1000, 4000]);
        m.extend(vec![0u8; 80]);
        m
    });
    // A sound track pointing at the chapter track, a second one in Hebrew.
    let mut mp4a = vec![0u8; 16];
    mp4a.extend(2u16.to_be_bytes());
    mp4a.extend(vec![0u8; 10]);
    moov.extend(mp4_trak(
        1,
        b"soun",
        b"mp4a",
        &mp4a,
        "eng",
        &[],
        at,
        1000,
        Some(4),
    ));
    moov.extend(mp4_trak(
        2,
        b"soun",
        b"mp4a",
        &mp4a,
        "heb",
        &[],
        at,
        1000,
        None,
    ));
    moov.extend(mp4_trak(
        3,
        b"sbtl",
        b"tx3g",
        &[0u8; 40],
        "fre",
        &sizes(&subs),
        at,
        1500,
        None,
    ));
    let chap_at = at + subs.iter().map(|s| s.len() as u32).sum::<u32>();
    moov.extend(mp4_trak(
        4,
        b"text",
        b"text",
        &[0u8; 40],
        "eng",
        &sizes(&chaps),
        chap_at,
        2000,
        None,
    ));
    f.extend(bx(b"moov", &moov));
    let x = read(&f);
    assert_eq!(x.audio.len(), 2);
    assert_eq!(x.audio[1].lang.as_deref(), Some("heb"));
    assert!(x.audio[0].default && !x.audio[1].default);
    assert_eq!(
        x.subtitles.len(),
        1,
        "the chapter track is not a subtitle track"
    );
    let s = &x.subtitles[0];
    assert_eq!(
        (s.info.codec.as_str(), s.info.lang.as_deref()),
        ("tx3g", Some("fre"))
    );
    assert_eq!(s.packets.len(), 2);
    assert_eq!(
        (s.packets[1].start_us, s.packets[1].dur_us),
        (1_500_000, Some(1_500_000))
    );
    assert_eq!(&s.packets[0].data[2..], b"Hello");
    assert_eq!(x.chapters.len(), 2);
    assert_eq!(x.chapters[1].start_us, 2_000_000);
    assert_eq!(x.chapters[1].name("en"), Some("Middle"));
}

#[test]
fn nero_chapter_list() {
    let mut chpl = vec![0u8; 4];
    chpl.push(2);
    for (t, n) in [(0u64, "One"), (90_000_000u64, "Two")] {
        chpl.extend(t.to_be_bytes());
        chpl.push(n.len() as u8);
        chpl.extend_from_slice(n.as_bytes());
    }
    let moov = bx(b"udta", &bx(b"chpl", &chpl));
    let c = nero_chapters(&moov);
    assert_eq!(c.len(), 2);
    assert_eq!(c[1].start_us, 9_000_000);
    assert_eq!(c[1].name(""), Some("Two"));
}

#[test]
fn language_codes() {
    assert!(lang_matches("heb", "he"));
    assert!(lang_matches("he-IL", "he"));
    assert!(lang_matches("eng", "en"));
    assert!(!lang_matches("ara", "he"));
    assert_eq!(iso639_2("ger"), Some("de"));
}

#[test]
fn accessibility_flags() {
    // Matroska: the hearing and visual impaired flags; names that say so.
    let mut tracks = mkv_track(1, 2, "A_AAC", "eng", &[]);
    tracks.extend(mkv_track(
        2,
        2,
        "A_AAC",
        "eng",
        &uel(FLAG_VISUAL_IMPAIRED, 1),
    ));
    tracks.extend(mkv_track(
        3,
        2,
        "A_AAC",
        "eng",
        &el(NAME, b"English (Audio Description)"),
    ));
    tracks.extend(mkv_track(
        4,
        0x11,
        "S_TEXT/UTF8",
        "eng",
        &uel(FLAG_HEARING_IMPAIRED, 1),
    ));
    tracks.extend(mkv_track(
        5,
        0x11,
        "S_TEXT/UTF8",
        "eng",
        &el(NAME, b"English SDH"),
    ));
    tracks.extend(mkv_track(
        6,
        0x11,
        "S_TEXT/UTF8",
        "eng",
        &el(NAME, b"Forced"),
    ));
    tracks.extend(mkv_track(
        7,
        0x11,
        "S_TEXT/UTF8",
        "eng",
        &el(NAME, b"Adventure"),
    ));
    let mut seg = el(INFO, &uel(TIMESTAMP_SCALE, 1_000_000));
    seg.extend(el(TRACKS, &tracks));
    let mut f = header("matroska");
    f.extend(el(SEGMENT, &seg));
    let x = read(&f);
    let ad: Vec<bool> = x.audio.iter().map(|a| a.visual_impaired).collect();
    assert_eq!(ad, [false, true, true]);
    let s: Vec<(bool, bool)> = x
        .subtitles
        .iter()
        .map(|s| (s.info.hearing_impaired, s.info.forced))
        .collect();
    assert_eq!(
        s,
        [(true, false), (true, false), (false, true), (false, false)]
    );

    // MP4: Apple's characteristics and ISO kind roles.
    let mut udta = bx(b"tagc", b"public.accessibility.describes-music-and-sound");
    udta.extend(bx(b"tagc", b"public.subtitles.forced-only"));
    let trak = bx(b"udta", &udta);
    assert_eq!(udta_flags(&trak), (true, true, false));
    let mut kind = vec![0u8; 4];
    kind.extend(b"urn:mpeg:dash:role:2011\0description\0");
    let trak = bx(b"udta", &bx(b"kind", &kind));
    assert_eq!(udta_flags(&trak), (false, false, true));
}

#[test]
fn mp4_closed_caption_track() {
    // A c608 track: CC1 pop-on "HI", one pair a sample.
    let p = |b: u8| {
        if b.count_ones().is_multiple_of(2) {
            b | 0x80
        } else {
            b
        }
    };
    let pairs = [
        [p(0x14), p(0x20)],
        [p(0x14), p(0x20)],
        [p(b'H'), p(b'I')],
        [p(0x14), p(0x2F)],
        [p(0x14), p(0x2F)],
    ];
    let samples: Vec<Vec<u8>> = pairs
        .iter()
        .map(|pr| {
            let mut s = 10u32.to_be_bytes().to_vec();
            s.extend(b"cdat");
            s.extend(pr);
            s
        })
        .collect();
    let mut f = bx(b"ftyp", b"isom\0\0\0\0isom");
    let at = f.len() as u32 + 8;
    f.extend(bx(b"mdat", &samples.concat()));
    let mut moov = full(b"mvhd", 0, &{
        let mut m = be32(&[0, 0, 1000, 4000]);
        m.extend(vec![0u8; 80]);
        m
    });
    let sizes: Vec<u32> = samples.iter().map(|s| s.len() as u32).collect();
    moov.extend(mp4_trak(
        1, b"clcp", b"c608", &[0u8; 8], "eng", &sizes, at, 40, None,
    ));
    f.extend(bx(b"moov", &moov));
    let x = read(&f);
    assert_eq!(x.subtitles.len(), 1);
    let s = &x.subtitles[0];
    assert_eq!(
        (s.info.codec.as_str(), s.private.as_slice()),
        ("CEA-608", &[1u8][..])
    );
    assert!(s.info.hearing_impaired);
    assert_eq!(s.info.id, crate::captions::ID_608 + 1);
    assert_eq!(s.packets.len(), 5);
    assert_eq!(s.packets[2].start_us, 80_000);
    assert_eq!(s.packets[2].data, [0, p(b'H'), p(b'I')]);
}
