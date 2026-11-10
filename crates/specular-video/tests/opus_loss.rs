//! Opus packets lost on the way (a network stream losing pages, a damaged file): rebuilt
//! from the next packet's in-band FEC data, else concealed, in the player and in a whole
//! decode. Fixture: tests/data/gen_fixtures_opus_fec.sh.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_video::audio::{self, FakeOutput};
use specular_video::mp4::{AudioCodec, AudioTrack};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// The Ogg pages of `d`: (start, end).
fn ogg_pages(d: &[u8]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i + 27 <= d.len() && &d[i..i + 4] == b"OggS" {
        let n = usize::from(d[i + 26]);
        let len: usize = d[i + 27..i + 27 + n].iter().map(|&l| usize::from(l)).sum();
        v.push((i, i + 27 + n + len));
        i += 27 + n + len;
    }
    v
}

/// The fixture without the pages of packets `lost` (pages 0 and 1 are the headers; packet
/// k is page k + 2, one packet per page).
fn without(whole: &[u8], lost: &[usize]) -> Vec<u8> {
    let mut cut = Vec::new();
    for (k, &(a, b)) in ogg_pages(whole).iter().enumerate() {
        if !lost.iter().any(|&l| l + 2 == k) {
            cut.extend_from_slice(&whole[a..b]);
        }
    }
    cut
}

/// Signal to error ratio (dB) of `got` against `want` over `range`.
fn snr_db(want: &[f32], got: &[f32], range: std::ops::Range<usize>) -> f64 {
    let (mut s, mut e) = (0f64, 0f64);
    for i in range {
        let (w, g) = (f64::from(want[i]), f64::from(got[i]));
        s += w * w;
        e += (w - g) * (w - g);
    }
    10.0 * (s / e.max(1e-20)).log10()
}

fn decode(name: &str, bytes: Vec<u8>) -> (AudioTrack, Vec<f32>, Arc<Source>) {
    let src = Arc::new(Source::from_vec(name, bytes));
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.clone().unwrap();
    let (pcm, ch, _) = audio::decode_track(&t, &src).unwrap();
    assert_eq!(ch, 1);
    (t, pcm, src)
}

/// Pages lost as a network stream loses them (the sequence numbers jump): the timeline keeps
/// its length; each lost packet comes back from the next packet's FEC data, well above
/// concealment and as close to the intact decode as libopus gets (through ctypes, on the same
/// losses: `tests/data/opus_fec_oracle.py`); where two are lost in a row the first is
/// concealed, and where the next packet has no FEC data the lost one is too. The player plays
/// the same as a whole decode.
#[test]
fn opus_lost_packets_come_back_from_fec() {
    let whole = std::fs::read(data("snd-opus-fec.opus")).unwrap();
    let lost = [10usize, 20, 29, 30, 40, 44];
    let (t0, want, _) = decode("whole.opus", whole.clone());
    let (t1, got, src) = decode("lossy.opus", without(&whole, &lost));
    assert_eq!(
        t1.samples.len(),
        t0.samples.len(),
        "lost packets keep their places"
    );
    assert_eq!(t1.duration, t0.duration);
    for &l in &lost {
        assert_eq!(t1.samples[l].size, 0);
        assert_eq!(t1.samples[l].dts, t0.samples[l].dts);
    }
    assert_eq!(got.len(), want.len());

    // Concealment alone (no FEC data looked at), as a baseline.
    let AudioCodec::Opus(h) = &t0.codec else {
        panic!("not Opus");
    };
    let mut d = specular_opus::Decoder::new(&specular_opus::Head::parse(h).unwrap()).unwrap();
    let mut concealed = Vec::new();
    for (k, s) in t0.samples.iter().enumerate() {
        let b = t0.sample_data(&whole, s).unwrap();
        let b: &[u8] = if lost.contains(&k) { &[] } else { &b };
        d.decode(b, &mut concealed).unwrap();
    }
    // (A whole decode is not trimmed by the edit: sample times index it directly.)
    assert_eq!(concealed.len(), want.len());
    let concealed = &concealed[..];
    let window = |k: usize| {
        let a = t0.samples[k].dts as usize;
        a..(a + 960).min(want.len())
    };
    let snr: Vec<(usize, f64, f64)> = lost
        .iter()
        .map(|&l| {
            (
                l,
                snr_db(&want, &got, window(l)),
                snr_db(&want, concealed, window(l)),
            )
        })
        .collect();
    for &(l, fec, conc) in &snr {
        eprintln!("lost packet {l}: FEC {fec:.1} dB, concealed {conc:.1} dB");
    }
    for &(l, fec, conc) in &snr {
        if l == 29 {
            // The first of two in a row has no packet after it: concealed (the history
            // before differs from the baseline's, which concealed 10 and 20 too).
            assert!((fec - conc).abs() < 0.5, "{l}: {fec} vs {conc}");
        } else if l == 40 {
            // Packet 41 carries no FEC data (the encoder left it out; libopus's
            // `opus_packet_has_lbrr` agrees): concealed through the FEC path.
            assert!((fec - conc).abs() < 1.0, "{l}: {fec} vs {conc}");
        } else {
            // libopus's FEC decode against its intact decode over the same 20 ms.
            let libopus = match l {
                10 => 21.1,
                20 => 19.6,
                30 => 4.0,
                _ => 22.7,
            };
            assert!(
                fec > conc + 3.0 && (fec - libopus).abs() < 1.0,
                "{l}: FEC {fec} dB (libopus {libopus}), concealed {conc} dB"
            );
        }
    }
    let all = snr_db(&want, &got, 0..want.len());
    let all_conc = snr_db(&want, concealed, 0..want.len());
    eprintln!("whole second: FEC {all:.1} dB, concealed {all_conc:.1} dB");
    assert!(all > all_conc + 3.0);

    // The player.
    let out = FakeOutput::new(48_000, 1);
    let p = specular_video::session::open_sound(src, Box::new(out.clone())).unwrap();
    p.play(std::time::Duration::ZERO);
    let n = want.len() - 960;
    // (The player keeps only 300 ms ahead: pulled 100 ms at a time.)
    let mut pulled = 0;
    while pulled < n {
        let k = 4800.min(n - pulled);
        assert!(p.wait_queued(k));
        out.pull(k, std::time::Instant::now());
        pulled += k;
    }
    let played = out.played.lock().clone();
    assert!(played.len() >= n);
    // (The player starts at the edit: the pre-skip left out.)
    let skip = t0.edit.start as usize;
    let worst = played[..n - skip]
        .iter()
        .zip(&got[skip..])
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(
        worst < 1e-6,
        "the player differs from a whole decode by {worst}"
    );
}

/// A damaged packet (one that will not parse) is rebuilt from the next one's FEC data too,
/// keeping its 20 ms.
#[test]
fn opus_damaged_packet_comes_back_from_fec() {
    let whole = std::fs::read(data("snd-opus-fec.opus")).unwrap();
    let (t0, want, _) = decode("whole.opus", whole.clone());
    // Packet 25 becomes a code 3 packet claiming 48 frames (more than 120 ms: malformed),
    // same length, so the page stays valid.
    let s = &t0.samples[25];
    let at = (s.offset & ((1 << 40) - 1)) as usize;
    let skip = (s.offset >> 40) as usize;
    let body = at + 27 + usize::from(whole[at + 26]) + skip;
    let mut bad = whole.clone();
    bad[body] = (bad[body] & 0xFC) | 3;
    bad[body + 1] = 48;
    // (The page CRC is not checked by the demuxer, as a damaged file would have it wrong.)
    let (_, got, _) = decode("damaged.opus", bad);
    assert_eq!(got.len(), want.len());
    let a = s.dts as usize;
    let fec = snr_db(&want, &got, a..a + 960);
    eprintln!("damaged packet 25: FEC {fec:.1} dB");
    assert!(fec > 8.0, "{fec} dB");
}
