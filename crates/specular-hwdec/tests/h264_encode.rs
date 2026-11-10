//! Hardware H.264 encoding (VA-API on Linux, Media Foundation on Windows): a synthetic clip
//! encoded on the hardware, decoded by ffmpeg when it is installed (an independent oracle),
//! else by our decoder, and compared with the source frames. Skipped without a hardware H.264
//! encoder; `SPECULAR_REQUIRE_VAAPI_ENC=1` makes a missing encoder a failure.

use specular_hwdec::encode::{EncodeConfig, H264Encoder, avcc, pps_id, sps_id};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn required() -> bool {
    std::env::var("SPECULAR_REQUIRE_VAAPI_ENC").is_ok_and(|v| v == "1")
}

fn skip(why: &str) {
    assert!(!required(), "SPECULAR_REQUIRE_VAAPI_ENC=1 but {why}");
    eprintln!("skipped: {why}");
}

fn scratch() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("h264-encode");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A planar 4:2:0 picture: a diagonal gradient with fine detail, panned 3 px a frame, and a
/// moving block, so P pictures have motion to find.
fn frame(w: usize, h: usize, t: usize) -> [Vec<u8>; 3] {
    let (cw, ch) = (w / 2, h / 2);
    let mut y = vec![0u8; w * h];
    for r in 0..h {
        for c in 0..w {
            let x = c + 3 * t;
            let v = (x + r) / 8 + ((x / 4 + r / 4) % 2) * 24 + ((x * 7 + r * 3) % 13);
            y[r * w + c] = (16 + v % 200) as u8;
        }
    }
    let (bx, by) = ((40 + 9 * t) % (w - 64), (30 + 5 * t) % (h - 64));
    for r in by..by + 64 {
        y[r * w + bx..r * w + bx + 64].fill(220);
    }
    let mut u = vec![0u8; cw * ch];
    let mut v = vec![0u8; cw * ch];
    for r in 0..ch {
        for c in 0..cw {
            u[r * cw + c] = (64 + (c + t) * 128 / cw) as u8;
            v[r * cw + c] = (64 + r * 128 / ch) as u8;
        }
    }
    [y, u, v]
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let mse = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| f64::from(i32::from(x) - i32::from(y)).powi(2))
        .sum::<f64>()
        / a.len() as f64;
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The clip as raw yuv420p frames, decoded by ffmpeg from an Annex B stream (parameter sets
/// in front of every IDR picture, as a raw .h264 needs them).
fn decode_ffmpeg(
    w: usize,
    h: usize,
    sps: &[u8],
    pps: &[u8],
    samples: &[(Vec<u8>, bool)],
) -> Vec<u8> {
    let mut stream = Vec::new();
    for (data, key) in samples {
        if *key {
            for ps in [sps, pps] {
                stream.extend_from_slice(&[0, 0, 0, 1]);
                stream.extend_from_slice(ps);
            }
        }
        let mut i = 0;
        while i + 4 <= data.len() {
            let len = u32::from_be_bytes(data[i..i + 4].try_into().unwrap()) as usize;
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&data[i + 4..i + 4 + len]);
            i += 4 + len;
        }
        assert_eq!(i, data.len(), "AVCC lengths");
    }
    let dir = scratch();
    let es = dir.join(format!("hw_{w}x{h}.h264"));
    let yuv = dir.join(format!("hw_{w}x{h}.yuv"));
    std::fs::write(&es, &stream).unwrap();
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&es)
        .args(["-f", "rawvideo", "-pix_fmt", "yuv420p"])
        .arg(&yuv)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "ffmpeg: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dec = std::fs::read(&yuv).unwrap();
    let _ = std::fs::remove_file(&yuv);
    dec
}

/// The clip as raw yuv420p frames, decoded by our decoder from the samples and `avcC`.
fn decode_ours(avcc: &[u8], samples: &[(Vec<u8>, bool)]) -> Vec<u8> {
    let opts = specular_h264::Options {
        threads: 1,
        output: specular_h264::OutputOrder::Display,
    };
    let mut d = specular_h264::Decoder::with_avcc(avcc, opts).unwrap();
    let mut pictures = Vec::new();
    for (i, (data, _)) in samples.iter().enumerate() {
        d.decode_sample(data, i as i64)
            .unwrap_or_else(|e| panic!("our decoder, sample {i}: {e}"));
        while let Some(p) = d.next_picture(false) {
            pictures.push(p);
        }
    }
    d.flush();
    while let Some(p) = d.next_picture(true) {
        pictures.push(p);
    }
    assert_eq!(pictures.len(), samples.len(), "our decoder's picture count");
    pictures.sort_by_key(|p| p.tag);
    pictures.iter().flat_map(|p| p.planes.concat()).collect()
}

/// Encodes `n` frames, decodes them and checks every frame's PSNR and the IDR cadence.
fn round_trip(w: usize, h: usize, n: usize, qp: u8, min_db: f64) {
    let caps = specular_hwdec::capabilities().and_then(|c| c.h264_encode.clone());
    if caps.is_none() {
        skip("no hardware H.264 encoder");
        return;
    }
    let cfg = EncodeConfig {
        width: w as u32,
        height: h as u32,
        fps_num: 30,
        fps_den: 1,
        qp,
        keyint: 12,
    };
    let t0 = Instant::now();
    let mut enc = H264Encoder::new(&cfg).expect("the probe found an encoder, so it must open");
    eprintln!(
        "{} ({:?}) opened in {:?}",
        enc.device(),
        enc.profile(),
        t0.elapsed()
    );
    assert_eq!(sps_id(enc.sps()), Some(0));
    assert_eq!(pps_id(enc.pps()), Some(0));
    let avcc = avcc(&[enc.sps()], &[enc.pps()]);
    let mut samples = Vec::new();
    let mut src = Vec::new();
    let mut keys = 0;
    let t0 = Instant::now();
    for t in 0..n {
        let [y, u, v] = frame(w, h, t);
        let p = enc
            .encode(&y, &u, &v, w, w / 2, t == 20)
            .unwrap_or_else(|e| panic!("frame {t}: {e}"));
        // Every 12th from the last IDR; the forced one at 20 restarts the count.
        assert_eq!(p.key, matches!(t, 0 | 12 | 20 | 32), "frame {t}");
        keys += usize::from(p.key);
        samples.push((p.data, p.key));
        src.push([y, u, v]);
    }
    let took = t0.elapsed();
    assert!(keys >= 2);
    let bytes: usize = samples.iter().map(|s| s.0.len()).sum();
    let (dec, oracle) = if have_ffmpeg() {
        (
            decode_ffmpeg(w, h, enc.sps(), enc.pps(), &samples),
            "ffmpeg",
        )
    } else {
        (decode_ours(&avcc, &samples), "our decoder")
    };
    let fsize = w * h + 2 * (w.div_ceil(2) * h.div_ceil(2));
    assert_eq!(
        dec.len(),
        fsize * n,
        "{oracle} decoded a different frame count"
    );
    let mut worst = f64::MAX;
    for (t, s) in src.iter().enumerate() {
        let d = &dec[t * fsize..][..fsize];
        let cs = w.div_ceil(2) * h.div_ceil(2);
        let py = psnr(&s[0], &d[..w * h]);
        let pu = psnr(&s[1], &d[w * h..w * h + cs]);
        let pv = psnr(&s[2], &d[w * h + cs..]);
        worst = worst.min(py.min(pu).min(pv));
        assert!(
            py >= min_db && pu >= min_db && pv >= min_db,
            "frame {t}: PSNR Y {py:.1} U {pu:.1} V {pv:.1} dB"
        );
    }
    eprintln!(
        "{w}×{h} qp {qp}: {n} frames in {took:?} ({:.0} fps), {bytes} bytes, worst plane PSNR \
         {worst:.1} dB against {oracle}",
        n as f64 / took.as_secs_f64()
    );
}

#[test]
fn encodes_1080p_and_decodes() {
    // 1080 is not a multiple of 16: the bottom macroblock row is cropped.
    round_trip(1920, 1080, 36, 24, 34.0);
}

#[test]
fn encodes_small_odd_macroblock_size() {
    round_trip(328, 186, 16, 20, 36.0);
}

/// The hardware's own H.264 clip, decoded by the hardware decoder: every picture passes the
/// checks before the driver (`specular_hwdec::gate`) and comes out close to the source.
#[test]
fn hardware_decoder_takes_the_encoders_clip() {
    use specular_hwdec::video::{VideoCodec, VideoDecoder, video_supported};
    let (w, h, n) = (320usize, 176usize, 30usize);
    specular_hwdec::set_enabled(true);
    if specular_hwdec::capabilities()
        .and_then(|c| c.h264_encode.clone())
        .is_none()
    {
        skip("no hardware H.264 encoder");
        return;
    }
    if !video_supported(VideoCodec::H264) {
        eprintln!("skipped: no hardware H.264 decoder");
        return;
    }
    let cfg = EncodeConfig {
        width: w as u32,
        height: h as u32,
        fps_num: 30,
        fps_den: 1,
        qp: 22,
        keyint: 15,
    };
    let mut enc = H264Encoder::new(&cfg).expect("encoder");
    let config = avcc(&[enc.sps()], &[enc.pps()]);
    let mut dec = VideoDecoder::new(VideoCodec::H264, &config).expect("decoder");
    let mut src = Vec::new();
    let mut out = Vec::new();
    for t in 0..n {
        let [y, u, v] = frame(w, h, t);
        let p = enc.encode(&y, &u, &v, w, w / 2, false).expect("encode");
        dec.decode(&p.data, t as i64)
            .unwrap_or_else(|e| panic!("hardware decode of frame {t}: {e}"));
        while let Some(f) = dec.next_output() {
            out.push(f);
        }
        src.push(y);
    }
    dec.flush();
    while let Some(f) = dec.next_output() {
        out.push(f);
    }
    assert_eq!(out.len(), n, "pictures out of the hardware decoder");
    for (tag, f) in &out {
        assert_eq!((f.width, f.height), (w as u32, h as u32));
        let db = psnr(&src[*tag as usize], &f.planes[0]);
        assert!(db >= 34.0, "frame {tag}: Y PSNR {db:.1} dB");
    }
}
