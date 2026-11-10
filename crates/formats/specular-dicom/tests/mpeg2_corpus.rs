//! MPEG-2 endoscopy video (fo-dicom's ETIAM_video_002.dcm, from
//! tools/corpus/fetch-videocodecs.sh) against ffmpeg's decode of its stream as RGB: every
//! frame decoded front to back, a few after seeks, the levels compared. Skips without
//! `SPECULAR_VIDEOCODECS_CORPUS`.

use specular_dicom::Dicom;
use specular_dicom::pixel::Pixels;

#[test]
fn etiam_mpeg2_video_matches_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_VIDEOCODECS_CORPUS") else {
        return;
    };
    let base = std::path::Path::new(&dir).join("dicom/ETIAM_video_002");
    let file = std::fs::read(base.with_extension("dcm")).unwrap();
    let want = std::fs::read(base.with_extension("rgb")).unwrap();
    let d = Dicom::parse(&file).unwrap();
    d.check_supported().unwrap();
    let frame_bytes = 352 * 288 * 3;
    let t = std::time::Instant::now();
    let mut seq = d.video_sequence().unwrap();
    let (mut n, mut worst, mut sum, mut count) = (0usize, 0i32, 0f64, 0f64);
    while let Some(r) = seq.next_frame(d.buf(&file), None) {
        let (i, f) = r.unwrap();
        assert_eq!(i as usize, n);
        let Pixels::Rgb8(px) = &f.pixels else {
            panic!("RGB expected");
        };
        let w = &want[n * frame_bytes..(n + 1) * frame_bytes];
        for (a, b) in px.iter().zip(w) {
            let e = (i32::from(*a) - i32::from(*b)).abs();
            worst = worst.max(e);
            sum += f64::from(e);
            count += 1.0;
        }
        n += 1;
    }
    let el = t.elapsed().as_secs_f64();
    eprintln!(
        "{n} frames ({} declared, ffmpeg {}): {:.0} fps, within {worst} levels, mean {:.3}",
        d.frame_count(),
        want.len() / frame_bytes,
        n as f64 / el,
        sum / count
    );
    assert_eq!(n as u32, d.frame_count());
    assert!(n <= want.len() / frame_bytes);
    // Different YCbCr-to-RGB conversions (chroma interpolation, rounding): a few levels.
    assert!(
        worst <= 32 && sum / count < 1.0,
        "within {worst}, mean {}",
        sum / count
    );
    for k in [300u32, 17, 449, 0] {
        let a = d.decode_frame(&file, k, None).unwrap();
        let Pixels::Rgb8(px) = &a.pixels else {
            panic!("RGB expected");
        };
        let w = &want[k as usize * frame_bytes..(k as usize + 1) * frame_bytes];
        let e = px
            .iter()
            .zip(w)
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .max()
            .unwrap();
        assert!(e <= worst, "frame {k} after a seek: {e} levels off");
    }
}
