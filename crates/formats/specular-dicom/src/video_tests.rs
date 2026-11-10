//! H.264 / HEVC pixel data: random access against sequential decoding, fragmentation,
//! missing Number of Frames, accuracy against the source, refusals and damaged streams.

use super::*;
use crate::fixture;
use specular_h264enc as enc;

const W: u32 = 64;
const H: u32 = 48;

/// Frame `t` of the source: a luma ramp moving right by 3 samples a frame, chroma ramps.
fn source(t: u32) -> [Vec<u8>; 3] {
    let mut y = vec![0u8; (W * H) as usize];
    for r in 0..H {
        for c in 0..W {
            y[(r * W + c) as usize] = (40 + ((c + 3 * t) % W) * 2 + r) as u8;
        }
    }
    let (cw, ch) = (W / 2, H / 2);
    let mut u = vec![0u8; (cw * ch) as usize];
    let mut v = vec![0u8; (cw * ch) as usize];
    for r in 0..ch {
        for c in 0..cw {
            u[(r * cw + c) as usize] = (96 + c * 2) as u8;
            v[(r * cw + c) as usize] = (160 - r * 2) as u8;
        }
    }
    [y, u, v]
}

/// An Annex B H.264 stream of `n` frames with an IDR every `keyint`, plus the encoder's
/// reconstructed pictures in input order.
fn h264(n: u32, keyint: u32) -> (Vec<u8>, Vec<[Vec<u8>; 3]>) {
    let mut cfg = enc::Config::new(W, H, 25, 1, 18);
    cfg.keyint = keyint;
    let mut e = enc::Encoder::new(cfg).unwrap();
    let mut packets = Vec::new();
    let mut recon = Vec::new();
    for t in 0..n {
        let [y, u, v] = source(t);
        let f = enc::Frame {
            y: &y,
            u: &u,
            v: &v,
            y_stride: W as usize,
            c_stride: W as usize / 2,
        };
        packets.push(e.encode(&f, false));
        recon.push(e.reconstruction());
    }
    (enc::annex_b(&e.sps(), &e.pps(), &packets), recon)
}

fn rgb(f: &Frame) -> &[u8] {
    match &f.pixels {
        Pixels::Rgb8(p) => p,
        _ => panic!("expected RGB8"),
    }
}

/// BT.709 limited-range YCbCr to RGB, for checking the decoded colour.
fn bt709(y: u8, cb: u8, cr: u8) -> [f32; 3] {
    let y = (f32::from(y) - 16.0) * 255.0 / 219.0;
    let cb = (f32::from(cb) - 128.0) * 255.0 / 224.0;
    let cr = (f32::from(cr) - 128.0) * 255.0 / 224.0;
    [
        y + 1.5748 * cr,
        y - 0.1873 * cb - 0.4681 * cr,
        y + 1.8556 * cb,
    ]
}

fn video_file(stream: &[u8], fragment: usize, frames: Option<u32>) -> Vec<u8> {
    fixture::video(
        fixture::H264_HIGH_41,
        stream,
        fragment,
        W as u16,
        H as u16,
        frames,
        40.0,
        8,
    )
}

#[test]
fn h264_random_access_matches_sequential() {
    let (stream, recon) = h264(12, 4);
    let file = video_file(&stream, 0, Some(12));
    let d = Dicom::parse(&file).unwrap();
    d.check_supported().unwrap();
    assert_eq!(d.frame_count(), 12);
    let v = d.video().expect("indexed video");
    assert_eq!(v.codec(), video::Codec::H264);
    assert_eq!(v.frames(), 12);
    assert_eq!(v.restart_points(), 3);

    let mut seq = d.video_sequence().unwrap();
    let mut sequential = Vec::new();
    while let Some(r) = seq.next_frame(d.buf(&file), None) {
        let (i, f) = r.unwrap();
        assert_eq!(i as usize, sequential.len());
        assert_eq!((f.width, f.height), (W, H));
        sequential.push(rgb(&f).to_vec());
    }
    assert_eq!(sequential.len(), 12);

    // Each frame matches the encoder's reconstruction converted here (frame order kept).
    for (t, frame) in sequential.iter().enumerate() {
        let [y, u, v] = &recon[t];
        let mut worst = 0f32;
        for r in (0..H as usize).step_by(2) {
            for c in (0..W as usize).step_by(2) {
                let ci = (r / 2) * (W as usize / 2) + c / 2;
                let want = bt709(y[r * W as usize + c], u[ci], v[ci]);
                for k in 0..3 {
                    let got = f32::from(frame[(r * W as usize + c) * 3 + k]);
                    worst = worst.max((got - want[k].clamp(0.0, 255.0)).abs());
                }
            }
        }
        // Chroma interpolation makes up the rest; the frames differ by far more than this.
        assert!(worst <= 24.0, "frame {t}: {worst}");
    }
    assert_ne!(sequential[0], sequential[1]);

    // Random order, repeated, through the cache and restarts.
    for &i in &[7u32, 2, 11, 0, 5, 5, 4, 3, 10, 1, 9, 6, 8] {
        let f = d.decode_frame(&file, i, None).unwrap();
        assert_eq!(rgb(&f), &sequential[i as usize][..], "frame {i}");
    }
    assert!(d.video_memory() > 0);
    assert!(d.trim_video() > 0);
    assert_eq!(d.video_memory(), 0);
    let f = d.decode_frame(&file, 6, None).unwrap();
    assert_eq!(rgb(&f), &sequential[6][..]);

    // Rewinding replays the same frames.
    seq.rewind();
    let (i, f) = seq.next_frame(d.buf(&file), None).unwrap().unwrap();
    assert_eq!((i, rgb(&f)), (0, &sequential[0][..]));
}

/// Pixel data holding an MP4 file (not an Annex B stream) decodes like the stream itself,
/// whole or in fragments, in order and at random.
#[test]
fn h264_in_an_mp4_file() {
    let (stream, _) = h264(10, 4);
    let units = video::split_for_tests(&stream);
    let (sps, pps) = (units[0], units[1]);
    assert_eq!((sps[0] & 31, pps[0] & 31), (7, 8));
    let mut samples: Vec<Vec<&[u8]>> = Vec::new();
    for u in &units[2..] {
        if matches!(u[0] & 31, 1 | 5) || samples.is_empty() {
            samples.push(Vec::new());
        }
        samples.last_mut().unwrap().push(u);
    }
    assert_eq!(samples.len(), 10);
    let want: Vec<Vec<u8>> = {
        let file = video_file(&stream, 0, Some(10));
        let d = Dicom::parse(&file).unwrap();
        (0..10)
            .map(|i| rgb(&d.decode_frame(&file, i, None).unwrap()).to_vec())
            .collect()
    };
    for (fragment, co64) in [(0, false), (1000, true)] {
        let mp4 = crate::mp4::build::h264(sps, pps, &samples, co64);
        let file = video_file(&mp4, fragment, None);
        let d = Dicom::parse(&file).unwrap();
        d.check_supported().unwrap();
        assert_eq!(d.frame_count(), 10);
        assert_eq!(d.video().unwrap().restart_points(), 3);
        let mut seq = d.video_sequence().unwrap();
        let mut n = 0;
        while let Some(r) = seq.next_frame(d.buf(&file), None) {
            let (i, f) = r.unwrap();
            assert_eq!(rgb(&f), &want[i as usize][..], "frame {i}");
            n += 1;
        }
        assert_eq!(n, 10);
        for i in [9u32, 3, 6, 0] {
            assert_eq!(
                rgb(&d.decode_frame(&file, i, None).unwrap()),
                &want[i as usize][..]
            );
        }
    }
    // HEVC labelled, H.264 inside: refused with the reason.
    let mp4 = crate::mp4::build::h264(sps, pps, &samples, false);
    let file = fixture::video(
        fixture::HEVC_MAIN,
        &mp4,
        0,
        W as u16,
        H as u16,
        None,
        40.0,
        8,
    );
    let e = Dicom::parse(&file)
        .and_then(|d| d.check_supported())
        .unwrap_err();
    assert!(e.to_string().contains("MP4"), "{e}");
}

#[test]
fn h264_fragmentation_and_missing_frame_count() {
    let (stream, _) = h264(6, 3);
    let whole = video_file(&stream, 0, Some(6));
    let d = Dicom::parse(&whole).unwrap();
    let want: Vec<Vec<u8>> = (0..6)
        .map(|i| rgb(&d.decode_frame(&whole, i, None).unwrap()).to_vec())
        .collect();
    for (frag, frames) in [(6, Some(6)), (1000, Some(6)), (10, None), (0, Some(1))] {
        let file = fixture::video(
            fixture::H264_HIGH_41_FRAGMENTABLE,
            &stream,
            frag,
            W as u16,
            H as u16,
            frames,
            40.0,
            8,
        );
        let d = Dicom::parse(&file).unwrap();
        d.check_supported().unwrap();
        // A missing (or too large) Number of Frames is taken from the stream.
        assert_eq!(
            d.frame_count(),
            frames.unwrap_or(6),
            "fragment {frag}, frames {frames:?}"
        );
        for i in [5u32, 0, 3].into_iter().filter(|&i| i < d.frame_count()) {
            let f = d.decode_frame(&file, i, None).unwrap();
            assert_eq!(rgb(&f), &want[i as usize][..], "fragment {frag} frame {i}");
        }
    }
    let durations = d.frame_durations(&whole).unwrap();
    assert_eq!(durations.len(), 6);
    assert_eq!(durations[0], std::time::Duration::from_millis(40));
}

#[test]
fn video_rows_and_columns_crop_or_extend_the_picture() {
    let (stream, _) = h264(2, 2);
    let small = fixture::video(fixture::H264_HIGH_41, &stream, 0, 40, 30, Some(2), 40.0, 8);
    let f = Dicom::parse(&small)
        .unwrap()
        .decode_frame(&small, 1, None)
        .unwrap();
    assert_eq!((f.width, f.height, rgb(&f).len()), (40, 30, 40 * 30 * 3));
    let big = fixture::video(fixture::H264_HIGH_41, &stream, 0, 70, 50, Some(2), 40.0, 8);
    let f = Dicom::parse(&big)
        .unwrap()
        .decode_frame(&big, 0, None)
        .unwrap();
    assert_eq!((f.width, f.height), (70, 50));
    // Edge-extended: the last column repeats the picture's last column.
    let p = rgb(&f);
    let at = |x: usize, y: usize| &p[(y * 70 + x) * 3..(y * 70 + x) * 3 + 3];
    assert_eq!(at(69, 10), at(63, 10));
    assert_eq!(at(10, 49), at(10, 47));
}

/// The parameter sets and the length-prefixed slice data of a single-image HEIC.
fn heic_stream(heic: &[u8]) -> Vec<u8> {
    let find = |name: &[u8]| -> &[u8] {
        let at = heic.windows(4).position(|w| w == name).unwrap();
        let size = u32::from_be_bytes(heic[at - 4..at].try_into().unwrap()) as usize;
        &heic[at + 4..at - 4 + size]
    };
    let hvcc = hevc::hvcc::parse(find(b"hvcC")).unwrap();
    let mut out = Vec::new();
    for nal in &hvcc.nal_units {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(nal);
    }
    for nal in hevc::hvcc::split(find(b"mdat"), hvcc.length_size).unwrap() {
        out.extend_from_slice(&[0, 0, 1]);
        out.extend_from_slice(nal);
    }
    out
}

#[test]
fn hevc_frames_8_and_10_bit() {
    for (heic, ts, bits) in [
        (
            &include_bytes!("../../specular-heic/fuzz/seeds/c420.heic")[..],
            fixture::HEVC_MAIN,
            8u16,
        ),
        (
            &include_bytes!("../../specular-heic/fuzz/seeds/c420_10.heic")[..],
            fixture::HEVC_MAIN10,
            10,
        ),
    ] {
        let one = heic_stream(heic);
        let info = {
            let file = fixture::video(ts, &one, 0, 8, 8, Some(1), 40.0, bits);
            let d = Dicom::parse(&file).unwrap();
            d.check_supported().unwrap();
            let f = d.decode_frame(&file, 0, None).unwrap();
            (f.width, f.height)
        };
        // Three IDR pictures in a row: three frames; size from the stream itself.
        let stream = [&one[..], &one[..], &one[..]].concat();
        let (pw, ph) = probe_size(&one);
        let file = fixture::video(ts, &stream, 0, pw as u16, ph as u16, None, 40.0, bits);
        let d = Dicom::parse(&file).unwrap();
        assert_eq!(d.frame_count(), 3);
        assert_eq!(d.video().unwrap().codec(), video::Codec::Hevc);
        let f0 = d.decode_frame(&file, 0, None).unwrap();
        let f2 = d.decode_frame(&file, 2, None).unwrap();
        assert_eq!((f0.width, f0.height), (pw, ph));
        assert_eq!(f0.pixels, f2.pixels);
        assert_eq!(f0.bits, bits);
        match (&f0.pixels, bits) {
            (Pixels::Rgb8(_), 8) | (Pixels::Rgb16(_), 10) => {}
            _ => panic!("unexpected pixel kind for {bits}-bit HEVC"),
        }
        assert_eq!(info, (8, 8));
        let mut seq = d.video_sequence().unwrap();
        let mut n = 0;
        while let Some(r) = seq.next_frame(d.buf(&file), None) {
            assert_eq!(r.unwrap().1.pixels, f0.pixels);
            n += 1;
        }
        assert_eq!(n, 3);
    }
}

fn probe_size(annexb: &[u8]) -> (u32, u32) {
    let nals = video::split_for_tests(annexb);
    let params: Vec<&[u8]> = nals
        .iter()
        .filter(|n| (32..=34).contains(&((n[0] >> 1) & 63)))
        .copied()
        .collect();
    let i = hevc::probe(&params).unwrap();
    (i.width, i.height)
}

/// MPEG-2 (synthetic, ffmpeg-encoded: I/P/B, open GOPs of 6, 176x144) decodes frame for
/// frame as `specular-mpeg2` does, seeks land on the same frames as decoding front to back,
/// and the colour is BT.601 (SD) of the decoder's YCbCr.
#[test]
fn mpeg2_decodes_and_seeks() {
    let stream = include_bytes!("../../specular-mpeg2/tests/data/ibp.m2v");
    let (w, h) = (176usize, 144usize);
    let file = fixture::video(
        fixture::MPEG2_MAIN,
        stream,
        4000,
        w as u16,
        h as u16,
        Some(12),
        40.0,
        8,
    );
    let d = Dicom::parse(&file).unwrap();
    d.check_supported().unwrap();
    let v = d.video().expect("indexed video");
    assert_eq!(v.codec(), video::Codec::Mpeg2);
    assert_eq!(v.frames(), 12);
    // The three I pictures.
    assert_eq!(v.restart_points(), 3);
    let mut seq = d.video_sequence().unwrap();
    let mut sequential = Vec::new();
    while let Some(r) = seq.next_frame(d.buf(&file), None) {
        let (i, f) = r.unwrap();
        assert_eq!(i as usize, sequential.len());
        sequential.push(rgb(&f).to_vec());
    }
    assert_eq!(sequential.len(), 12);
    for i in [7u32, 1, 11, 0, 6, 5] {
        let f = d.decode_frame(&file, i, None).unwrap();
        assert!(
            rgb(&f) == &sequential[i as usize][..],
            "frame {i} after a seek"
        );
    }
    // Against the decoder's own pictures, converted with BT.601 here.
    let mut m = specular_mpeg2::Decoder::new(1, specular_mpeg2::OutputOrder::Display);
    m.decode(stream, 0).unwrap();
    m.flush();
    let mut k = 0;
    while let Some(p) = m.next_picture() {
        let mut worst = 0f32;
        for r in (0..h).step_by(2) {
            for c in (0..w).step_by(2) {
                let ci = (r / 2) * (w / 2) + c / 2;
                // Where chroma is flat around the pixel (the conversion interpolates it).
                let flat = |pl: &[u8]| {
                    let (a, cw) = (pl[ci], w / 2);
                    [ci + 1, ci + cw, ci.saturating_sub(cw)]
                        .iter()
                        .all(|&n| pl.get(n).is_none_or(|&b| b == a))
                };
                if !flat(&p.planes[1]) || !flat(&p.planes[2]) {
                    continue;
                }
                let (y, cb, cr) = (
                    f32::from(p.planes[0][r * w + c]) - 16.0,
                    f32::from(p.planes[1][ci]) - 128.0,
                    f32::from(p.planes[2][ci]) - 128.0,
                );
                let y = y * 255.0 / 219.0;
                let (cb, cr) = (cb * 255.0 / 224.0, cr * 255.0 / 224.0);
                let want = [
                    y + 1.402 * cr,
                    y - 0.344_136 * cb - 0.714_136 * cr,
                    y + 1.772 * cb,
                ];
                for j in 0..3 {
                    let got = f32::from(sequential[k][(r * w + c) * 3 + j]);
                    worst = worst.max((got - want[j].clamp(0.0, 255.0)).abs());
                }
            }
        }
        assert!(worst <= 2.5, "frame {k}: {worst} levels off");
        k += 1;
    }
    assert_eq!(k, 12);
}

#[test]
fn mislabelled_streams_are_refused() {
    let mpeg2 = [0, 0, 1, 0xB3, 0x04, 0x00, 0x30, 0x13, 0, 0, 1, 0xB5, 0x14];
    let file = video_file(&mpeg2, 0, Some(1));
    let d = Dicom::parse(&file).unwrap();
    let e = d.check_supported().unwrap_err().to_string();
    assert!(e.contains("MPEG-2") && e.contains("H.264"), "{e}");

    let mpeg4 = [
        0, 0, 1, 0xB0, 0x01, 0, 0, 1, 0xB5, 0x09, 0, 0, 1, 0x20, 0x00,
    ];
    let file = video_file(&mpeg4, 0, Some(1));
    let e = Dicom::parse(&file)
        .unwrap()
        .check_supported()
        .unwrap_err()
        .to_string();
    assert!(e.contains("MPEG-4"), "{e}");

    let file = video_file(b"no start code at all", 0, Some(1));
    assert!(Dicom::parse(&file).unwrap().check_supported().is_err());
}

#[test]
fn damaged_video_does_not_panic() {
    let (stream, _) = h264(5, 2);
    // Truncated streams.
    for cut in [
        0usize,
        3,
        10,
        30,
        stream.len() / 3,
        stream.len() / 2,
        stream.len() - 7,
    ] {
        let file = video_file(&stream[..cut.max(1)], 4, Some(5));
        let Ok(d) = Dicom::parse(&file) else { continue };
        for i in 0..d.frame_count().min(6) {
            let _ = d.decode_frame(&file, i, None);
        }
        if let Some(mut s) = d.video_sequence() {
            while s.next_frame(d.buf(&file), None).is_some() {}
        }
    }
    // Bytes flipped across the stream.
    let mut x = 0x2545_F491_u32;
    for _ in 0..40 {
        let mut s = stream.clone();
        for _ in 0..4 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % s.len();
            s[i] ^= (x >> 8) as u8 | 1;
        }
        let file = video_file(&s, 0, Some(5));
        if let Ok(d) = Dicom::parse(&file) {
            for i in [4u32, 0, 2] {
                let _ = d.decode_frame(&file, i, None);
            }
        }
    }
    // The whole file truncated at every few bytes, and a wrong fragment length.
    let file = video_file(&stream, 9, Some(5));
    for cut in (0..file.len()).step_by(97) {
        if let Ok(d) = Dicom::parse(&file[..cut]) {
            let _ = d.decode_frame(&file[..cut], 0, None);
        }
    }
    let mut bad = file.clone();
    let item = bad
        .windows(4)
        .rposition(|w| w == [0xFE, 0xFF, 0x00, 0xE0])
        .unwrap();
    bad[item + 4..item + 8].copy_from_slice(&0x7FFF_FFF0_u32.to_le_bytes());
    if let Ok(d) = Dicom::parse(&bad) {
        let _ = d.decode_frame(&bad, 4, None);
    }
    // A cancelled decode reports it.
    let d = Dicom::parse(&file).unwrap();
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert!(matches!(
        d.decode_frame(&file, 3, Some(&stop)),
        Err(Error::Cancelled)
    ));
}
